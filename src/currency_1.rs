use std::{
    fmt::Debug,
    sync::{Arc, Mutex},
    thread,
};

pub fn shared_data_1() {
    let shared_data = Arc::new(Mutex::new(vec![1, 2, 3]));
    let mut handlers: Vec<thread::JoinHandle<()>> = vec![];
    for _i in 0..3 {
        let inner_data = shared_data.clone();
        let handler = thread::spawn(move || {
            inner_data.lock().unwrap().push(4);
        });
        handlers.push(handler);
    }
    for handle in handlers {
        handle.join().unwrap(); // Wait for the thread to complete and handle any errors.
    }
    let data = shared_data.lock().unwrap();
    println!("data {:?}", *data);
}

// fn inner_data_1<T: Send + Sync + 'static + Debug>(val: T) {
//     let handler = thread::spawn(move || {
//         println!("data {:?}", val);
//     });
//     handler.join().unwrap();
// }

// pub fn shared_data_2() {
//     inner_data_1("mengru");
// }
