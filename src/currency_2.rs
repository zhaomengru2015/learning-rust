use crossbeam_channel::unbounded;
use std::collections::HashMap;
use std::sync::{Arc, Condvar, Mutex};
use std::thread;

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
    let handler = thread::spawn(move || loop {
        match worker_receiver.recv() {
            Ok(WorkMsg::Work(num)) => {
                let _ = result_sender.send(ResultMsg::Result(num + 100u8));
            }
            Ok(WorkMsg::Exit) => {
                let _ = result_sender.send(ResultMsg::Exited);
                break;
            }
            _ => panic!("Error receiving a WorkMsg"),
        }
    });

    let _ = worker_sender.send(WorkMsg::Work(1));
    let _ = worker_sender.send(WorkMsg::Work(2));
    let _ = worker_sender.send(WorkMsg::Exit);
    loop {
        match result_receiver.recv() {
            Ok(ResultMsg::Result(num)) => println!("received result {:?}", num),
            Ok(ResultMsg::Exited) => {
                println!("worker finished");
                break;
            }
            _ => panic!("Error receiving a ResultMsg"),
        }
    }
    handler.join().unwrap();
}
