use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
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
    let handler = thread::spawn(move || loop {
        // 使用crossbeam选择一个就绪的工作
        select! {
            recv(worker_receiver) -> msg =>{
                match msg {
                    Ok(WorkMsg::Work(num)) => {
                        let result_sender = result_sender.clone();
                        let pool_result_sender = pool_result_sender.clone();
                        let cache = cache.clone();
                        // 池上启动一个新的工作单元
                        worker_state.set_ongoing(1);
                        pool.spawn( move ||{
                            let num = {
                                // 缓存开始
                                let cache = cache.lock().unwrap();
                                let key = CacheKey(num);
                                if let Some(result) = cache.get(&key) {
                                    // 从缓存中获得一个结果，并将其发送回去，
                                    // 同时带有一个标志，表明是从缓存中获得了它
                                    let _ = result_sender.send(ResultMsg::Result(result.clone(), WorkPerformed::FromCache));
                                    let _ = pool_result_sender.send(());
                                    return;
                                }
                                key.0
                                // 缓存结束
                            };
                            //1. 发送结果给主组件
                            let _ = result_sender.send(ResultMsg::Result(num + 100u8,WorkPerformed::New));
                            // 在缓存中存储“昂贵”的work.
                            let mut cache = cache.lock().unwrap();
                            let key = CacheKey(num.clone());
                            cache.insert(key, num);
                            //2. 让并行组件知道这里完成了一个工作单元
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
                println!("pool_result_receiver received finished work {:?}", worker_state.ongoing);
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
