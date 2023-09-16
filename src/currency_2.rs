use std::thread;

use crossbeam_channel::unbounded;

enum WorkMsg {
    Work(u8),
    Exit,
}

enum ResultMsg {
    Result(u8),
    Exited,
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
    let mut ongoing_work = 0;
    let mut existing = false;
    let handler = thread::spawn(move || loop {
        // 使用crossbeam选择一个就绪的工作
        select! {
            recv(worker_receiver) -> msg =>{
                match msg {
                    Ok(WorkMsg::Work(num)) => {
                        let result_sender = result_sender.clone();
                        let pool_result_sender = pool_result_sender.clone();
                        // 池上启动一个新的工作单元
                        ongoing_work +=1;
                        pool.spawn( move ||{
                            //1. 发送结果给主组件
                            let _ = result_sender.send(ResultMsg::Result(num + 100u8));
                            //2. 让并行组件知道这里完成了一个工作单元
                            println!("worker finished work {:?}", ongoing_work);
                            let _ = pool_result_sender.send(());
                        });
                    }
                    Ok(WorkMsg::Exit) => {
                        existing  = true;
                        // 如果没有正在进行的工作，则退出
                        if ongoing_work == 0 {
                            let _ = result_sender.send(ResultMsg::Exited);
                            break;
                        }
                    }
                    _ => panic!("Error receiving a WorkMsg"),
                }
            },
            recv(pool_result_receiver) -> _ =>{
                println!("pool_result_receiver received finished work {:?}", ongoing_work);
                if ongoing_work == 0 {
                    panic!("Received an unexpected pool result");
                }
                //一个工作单元已经完成
                ongoing_work-=1;
                if ongoing_work == 0 && existing{
                    let _ = result_sender.send(ResultMsg::Exited);
                    break;
                }
            },
        }
    });

    let _ = worker_sender.send(WorkMsg::Work(1));
    let _ = worker_sender.send(WorkMsg::Work(2));
    let _ = worker_sender.send(WorkMsg::Exit);
    let mut counter = 0;
    loop {
        match result_receiver.recv() {
            Ok(ResultMsg::Result(num)) => {
                println!("received result {:?}", num);
                counter += 1;
            }
            Ok(ResultMsg::Exited) => {
                println!("worker finished");
                assert_eq!(2, counter);
                break;
            }
            _ => panic!("Error receiving a ResultMsg"),
        }
    }
    // handler.join().unwrap();
}
