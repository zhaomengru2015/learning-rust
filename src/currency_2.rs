use std::{
    collections::HashMap,
    sync::{Arc, Condvar, Mutex},
    thread,
};

use crossbeam_channel::unbounded;

enum WorkMsg {
    Work(u8),
    Exit,
}

enum ResultMsg {
    Result(u8, WorkPerformed),
    Exited,
}

#[derive(Clone, Debug)]
struct WorkerState {
    pub(crate) ongoing: i16,
    pub(crate) existing: bool,
}

impl WorkerState {
    fn init() -> Self {
        WorkerState {
            ongoing: 0,
            existing: false,
        }
    }
    fn set_ongoing(&mut self, count: i16) {
        self.ongoing += count;
    }
    fn set_existing(&mut self, existing: bool) {
        self.existing = existing;
    }
    fn unset_ongoing(&mut self, count: i16) {
        self.ongoing -= count;
    }
    fn is_existing(&self) -> bool {
        self.existing
    }
    fn is_no_more_work(&self) -> bool {
        self.ongoing == 0
    }
}

#[derive(Eq, Debug, PartialEq)]
enum WorkPerformed {
    FromCache,
    New,
}
#[derive(Eq, Hash, PartialEq)]
struct CacheKey(u8);

#[derive(Debug, Eq, PartialEq)]
enum CacheState {
    Ready,
    WorkInProgress,
}

// 主组件和子组件之间的通信
pub fn servo_channel_1() {
    let (worker_sender, worker_receiver) = unbounded();
    let (result_sender, result_receiver) = unbounded();
    let (pool_result_sender, pool_result_receiver) = unbounded();
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(2)
        .build()
        .unwrap();
    let mut worker_state = WorkerState {
        ongoing: 0,
        existing: false,
    };
    let cache: Arc<Mutex<HashMap<CacheKey, u8>>> = Arc::new(Mutex::new(HashMap::new()));
    // 增加缓存状态，指示对于给定的key，缓存是否已经准备好被读取。
    let cache_state: Arc<Mutex<HashMap<CacheKey, Arc<(Mutex<CacheState>, Condvar)>>>> =
        Arc::new(Mutex::new(HashMap::new()));
    let handler = thread::spawn(move || loop {
        // 使用crossbeam选择一个就绪的工作
        select! {
            recv(worker_receiver) -> msg =>{
                match msg {
                    Ok(WorkMsg::Work(num)) => {
                        let result_sender = result_sender.clone();
                        let pool_result_sender = pool_result_sender.clone();

                        // 使用缓存
                        let cache = cache.clone();
                        let cache_state = cache_state.clone();

                        // 池上启动一个新的工作单元
                        worker_state.set_ongoing(1);

                        pool.spawn( move ||{
                            let num = {
                                let (cache_state_lock, cvar) = {
                                    // cache_state 临界区开始
                                    let mut state_map = cache_state.lock().unwrap();
                                    &*state_map
                                    .entry(CacheKey(num.clone()))
                                    .or_insert_with(||{
                                        Arc::new((Mutex::new(CacheState::Ready), Condvar::new()))
                                    }).clone()
                                    //  `cache_state` 临界区结束
                                };

                                //state 临界区开始
                                let mut state =  cache_state_lock.lock().unwrap();

                                // 注意：使用while循环来防止条件变量的虚假唤醒
                                while let CacheState::WorkInProgress = *state {
                                    //阻塞直到状态为Ready
                                    let current_state = cvar.wait(state)
                                    .unwrap();
                                    state = current_state;
                                };

                                assert_eq!(*state, CacheState::Ready);
                                let (num, result) = {
                                    //缓存临界区开始
                                    let cache = cache.lock().unwrap();
                                    let key = CacheKey(num);
                                    let result = match cache.get(&key) {
                                        Some(result)=>Some(result.clone()),
                                        None=>None,
                                    };
                                    (key.0, result)
                                    // 缓存临界区结束
                                };
                                if let Some(result) = result {
                                    // 从缓存中获得一个结果，并将其发送回去，
                                    // 同时带有一个标志，表明是从缓存中获得了它
                                    let _ = result_sender.send(ResultMsg::Result(result, WorkPerformed::FromCache));
                                    let _ = pool_result_sender.send(());
                                    // 不要忘记通知等待线程
                                    cvar.notify_one();
                                    return;
                                }else{
                                    *state = CacheState::WorkInProgress;
                                    num
                                }
                                // `state` 临界区结束
                            };

                            //1. 在临界区外做更多「昂贵工作」
                            let _ = result_sender.send(ResultMsg::Result(num,WorkPerformed::New));
                            {
                                // 在缓存中存储“昂贵”的work.
                                let mut cache = cache.lock().unwrap();
                                let key = CacheKey(num.clone());
                                cache.insert(key, num);
                                // 缓存临界区结束
                            }

                            let (lock, cvar) = {
                                let mut state_map = cache_state.lock().unwrap();
                                &*state_map
                                    .get_mut(&CacheKey(num))
                                    .expect("Entry in cache state to have been previously inserted")
                                    .clone()
                            };

                            //重新进入state 临界区
                            let mut state = lock.lock().unwrap();
                            assert_eq!(*state, CacheState::WorkInProgress);
                            *state = CacheState::Ready;
                            cvar.notify_one();
                            let _ = pool_result_sender.send(());
                        });
                    }
                    Ok(WorkMsg::Exit) => {
                        worker_state.set_existing(true);
                        // 如果没有正在进行的工作，则退出
                        if worker_state.is_no_more_work() {
                            let _ = result_sender.send(ResultMsg::Exited);
                            break;
                        }
                    }
                    _ => panic!("Error receiving a WorkMsg"),
                }
            },
            recv(pool_result_receiver) -> _ =>{
                if worker_state.is_no_more_work() {
                    panic!("Received an unexpected pool result");
                }
                //一个工作单元已经完成
                worker_state.unset_ongoing(1);
                if worker_state.is_no_more_work() && worker_state.is_existing(){
                    let _ = result_sender.send(ResultMsg::Exited);
                    break;
                }
            },
        }
    });

    let _ = worker_sender.send(WorkMsg::Work(1));
    // 发送两个相同的work
    let _ = worker_sender.send(WorkMsg::Work(2));
    let _ = worker_sender.send(WorkMsg::Work(2));
    let _ = worker_sender.send(WorkMsg::Exit);
    let mut counter = 0;
    loop {
        match result_receiver.recv() {
            Ok(ResultMsg::Result(num, worker_performed)) => {
                println!(
                    "received result {:?}, worker_performed {:?}",
                    num, worker_performed
                );
                counter += 1;
            }
            Ok(ResultMsg::Exited) => {
                println!("worker finished");
                assert_eq!(3, counter);
                break;
            }
            _ => panic!("Error receiving a ResultMsg"),
        }
    }
    // handler.join().unwrap();
}
