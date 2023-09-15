use std::{thread, time::Duration};
fn main() {
    let duration = Duration::from_millis(3000);
    println!("main thread");
    let handler = thread::spawn(move || {
        thread::sleep(duration);
        println!("sub thread 1");
        // some work here
        let handler1 = thread::spawn(move || {
            thread::sleep(duration);
            println!("sub thread 2");
        });
        handler1.join().unwrap();
    });
    handler.join().unwrap();
}
