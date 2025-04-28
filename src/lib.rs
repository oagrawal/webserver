mod queue;
use std::{
    sync::{mpsc, Arc, Mutex},
    thread,
};
use crate::queue::ArrayQueue;


// Lock-based ThreadPool implementation
pub struct ThreadPool {
    workers: Vec<Worker>,
    sender: Option<mpsc::Sender<Job>>,
}

impl ThreadPool {
    pub fn new(size: usize) -> ThreadPool {
        assert!(size > 0);
        let (sender, receiver) = mpsc::channel();
        let receiver = Arc::new(Mutex::new(receiver));
        let mut workers = Vec::with_capacity(size);
        
        for id in 0..size {
            workers.push(Worker::new(id, Arc::clone(&receiver)));
        }
        
        ThreadPool {
            workers,
            sender: Some(sender),
        }
    }

    pub fn execute<F>(&self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let job = Box::new(f);
        self.sender.as_ref().unwrap().send(job).unwrap();
    }
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        drop(self.sender.take());
        
        for worker in &mut self.workers {
            println!("Shutting down worker {}", worker.id);
            
            if let Some(thread) = worker.thread.take() {
                thread.join().unwrap();
            }
        }
    }
}

type Job = Box<dyn FnOnce() + Send + 'static>;

struct Worker {
    id: usize,
    thread: Option<thread::JoinHandle<()>>,
}

impl Worker {
    fn new(id: usize, receiver: Arc<Mutex<mpsc::Receiver<Job>>>) -> Worker {
        let thread = thread::spawn(move || loop {
            let message = receiver.lock().unwrap().recv();
            
            match message {
                Ok(job) => {
                    println!("Worker {id} got a job; executing.");
                    job();
                }
                Err(_) => {
                    println!("Worker {id} disconnected; shutting down.");
                    break;
                }
            }
        });
        
        Worker { 
            id, 
            thread: Some(thread) 
        }
    }
}



pub struct LockFreeThreadPool {
    workers: Vec<LockFreeWorker>,
    job_queue: Arc<ArrayQueue<LockFreeJob>>,
    running: Arc<std::sync::atomic::AtomicBool>,
}

type LockFreeJob = Box<dyn FnOnce() + Send + 'static>;

struct LockFreeWorker {
    id: usize,
    thread: Option<thread::JoinHandle<()>>,
}

impl LockFreeThreadPool {
    pub fn new(size: usize, queue_capacity: usize) -> LockFreeThreadPool {
        assert!(size > 0);
        assert!(queue_capacity > 0);
        
        let job_queue = Arc::new(ArrayQueue::new(queue_capacity));
        let running = Arc::new(std::sync::atomic::AtomicBool::new(true));
        let mut workers = Vec::with_capacity(size);
        
        for id in 0..size {
            workers.push(LockFreeWorker::new(
                id,
                Arc::clone(&job_queue),
                Arc::clone(&running)
            ));
        }
        
        LockFreeThreadPool {
            workers,
            job_queue,
            running,
        }
    }

    pub fn execute<F>(&self, f: F) -> Result<(), ()>
    where
        F: FnOnce() + Send + 'static,
    {
        let job = Box::new(f);
        match self.job_queue.push(job) {
            Ok(()) => Ok(()),
            Err(_) => {
                eprintln!("Queue is full, rejecting job");
                Err(())
            },
        }
    }
}

impl Drop for LockFreeThreadPool {
    fn drop(&mut self) {
        self.running.store(false, std::sync::atomic::Ordering::SeqCst);

        for worker in &mut self.workers {
            if let Some(thread) = worker.thread.take() {
                println!("Shutting down worker {}", worker.id);
                thread.join().unwrap();
            }
        }
    }
}

impl LockFreeWorker {
    fn new(
        id: usize,
        job_queue: Arc<ArrayQueue<LockFreeJob>>,
        running: Arc<std::sync::atomic::AtomicBool>,
    ) -> LockFreeWorker {
        let thread = thread::spawn(move || {
            while running.load(std::sync::atomic::Ordering::SeqCst) {
                match job_queue.pop() {
                    Some(job) => {
                        println!("Worker {id} got a job; executing.");
                        job();
                    }
                    None => {
                        thread::yield_now();
                    }
                }
            }
            println!("Worker {id} shutting down.");
        });
        
        LockFreeWorker {
            id,
            thread: Some(thread),
        }
    }
}
