use std::{thread, time::Duration};

pub fn spawn_1() {
    println!("main thread");
    let handler = thread::spawn(move || {
        thread::sleep(Duration::from_millis(10000));
        println!("sub thread 1");
        let handler1 = thread::spawn(move || {
            thread::sleep(Duration::from_millis(20000));
            println!("sub thread 2");
        });
        // handler1.join().unwrap();
    });
    thread::sleep(Duration::from_millis(50000));
    // handler.join().unwrap();
}
