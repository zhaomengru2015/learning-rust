use std::sync::mpsc::{channel, Sender};
use std::sync::Arc;
use std::sync::Mutex;
use std::thread::JoinHandle;
pub struct ThreadPool {
    _handles: Vec<JoinHandle<()>>,
    sender: Sender<Box<dyn Fn() + Send>>,
}
impl ThreadPool {
    pub fn new(num_threads: u8) -> Self {
        let (sender, receiver) = channel::<Box<dyn Fn() + Send>>();
        let receiver = Arc::new(Mutex::new(receiver));
        let mut _handles: Vec<JoinHandle<()>> = vec![];
        for _ in 0..num_threads {
            let clone = receiver.clone();
            let handle: JoinHandle<()> = std::thread::spawn(move || loop {
                let work: Box<dyn Fn()> = match clone.lock().unwrap().recv() {
                    Ok(work) => work,
                    _ => break,
                };
                work();
            });
            _handles.push(handle);
        }
        Self { _handles, sender }
    }
    pub fn execute<T: Fn() + Send + 'static>(&self, work: T) {
        self.sender.send(Box::new(work)).unwrap();
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn it_works() {
        use std::sync::atomic::{AtomicU32, Ordering};
        let n = AtomicU32::new(0);
        let nref = Arc::new(n);
        let pool = ThreadPool::new(2);
        let clone = nref.clone();
        let foo = move || {
            nref.fetch_add(1, Ordering::SeqCst);
        };
        pool.execute(foo.clone());
        pool.execute(foo);
        std::thread::sleep(std::time::Duration::from_secs(1));
        assert_eq!(clone.load(Ordering::SeqCst), 2);
    }
}
