use std::{thread, sync::{mpsc::{self, Receiver}, Arc, Mutex}};

pub struct ThreadPool {
    workers: Vec<Worker>,
    transmitter: Option<mpsc::Sender<Job>>
}

// type alias: https://doc.rust-lang.org/book/ch19-04-advanced-types.html?highlight=type%20alias#creating-type-synonyms-with-type-aliases
type Job = Box<dyn FnOnce() -> () + Send + 'static>;

struct Worker {
    id: usize,
    thread: Option<thread::JoinHandle<()>>,
}

impl Worker {
    fn new(id: usize, receiver: Arc<Mutex<Receiver<Job>>>) -> Self {
        let thread = thread::spawn(move || loop {
            // not sure when the drop of the MutexGuard (by calling .lock().unwrap()) take place in this instance, which releases the lock
            
            // Note that lock() call blocks the thread until the lock is acquired
            
            // Note that recv() call also blocks the thread if no available data is received, entailing that the thread in the worker 
            // waits/is blocked until a job is received to be done

            // Note that the exact timing of the drop of MutexGuard (which equates to the release of the acquired lock) can be reasoned as following:
            
            // As given by the type hint, the lock() call returns a a MutexGuard smart pointer wrapped in a LockResult
            // s.t. the following unwrap() call gives the MutexGuard smart pointer. See similar reasoning: https://doc.rust-lang.org/book/ch16-03-shared-state.html#the-api-of-mutext
            
            // The MutexGuard smart pointer implements a Deref trait that get used with recv() call with the following reasoning:
            // As given bby the type hint, since recv() call requires &Receiver type, the MutexGuard<'_, Receiver<...>> returned
            // by the preceding unwrap() call is Deref coersed into &Receiver<...> to fit the type

            // Coming back to when the timing when the lock is dropped, since the values of the RHS of the let statement is dropped once the statement completes
            // we conclude that it is at this point, notably before the job runs, that the MutexGuard goes out of the scope
            // s.t. the Drop trait implementation of the MutexGuard releases the lock

            // See a subtly different behavior as explained in the book if the code is otherwise for the explanation:
            // while let Ok(job) = receiver.lock().unwrap().recv() {...}
            // https://doc.rust-lang.org/book/ch20-02-multithreaded.html#implementing-the-execute-method
            
            let job_msg = receiver.lock().unwrap().recv();

            match job_msg {
                Ok(job) => {
                    println!("Worker {id} got a job executing.");
                    job();
                }
                // given that the infinite loop of recv() call 
                // the Err arm is taking placing after the transmitter
                // is dropped
                Err(_) => {
                    println!("Worker {id} disconnected; shutting down.");
                    break;
                }
            }
        });
        
        Self {
            id,
            thread: Some(thread),
        }
    }
}

#[derive(Debug)]
pub enum PoolCreationError {
    ZeroThreadCreationError,
}

impl ThreadPool {
    /// Create a new ThreadPool.
    /// The size is the number of threads in the pool.
    pub fn build(num: usize) -> Result<ThreadPool, PoolCreationError> {
        
        if num == 0 {
            return Err(PoolCreationError::ZeroThreadCreationError);
        }

        // doing (known) vector space allocation up front is slightly more efficient than using Vec::new
        let mut workers = Vec::with_capacity(num);
        
        // Note that the sender is clonable s.t. they can be
        // moved into multiple spawned threads to send messages
        // simultaneously, see: https://doc.rust-lang.org/rust-by-example/std_misc/channels.html
        // The current use case IS NOT the above
        let (sender, receiver) = mpsc::channel();


        // Note that receiver can't be cloned and must be owned by one thread: https://doc.rust-lang.org/std/sync/mpsc/struct.Receiver.html
        // As such, here we'd like to share the ownership of ONE receiver among multiple threads
        // The atomic reference count (ARC) paired with mutual exclusion is a common solution pattern: https://doc.rust-lang.org/std/sync/mpsc/struct.Receiver.html
        // Arc<T> requires type T to be a Sync type s.t. references like &T are safe to share across threads
        // As such, since Mutex is a Sync type, Arc<Mutex> enables thread-safe interior mutability for ONE shared ownership
        let receiver = Arc::new(Mutex::new(receiver));

        for i in 0..num {
            let receiver = Arc::clone(&receiver);
            workers.push(Worker::new(i, receiver));
        }

        Ok(ThreadPool {workers, transmitter: Some(sender)})    
    }

    pub fn execute<F>(&self, f: F)
    where
        // An instance of using the trait bound
        // https://doc.rust-lang.org/book/ch10-02-traits.html?highlight=trait%20bound#using-trait-bounds-to-conditionally-implement-methods
        F: FnOnce() -> () + Send + 'static
    {
        let job = Box::new(f);
        
        // Use as_ref() to obtain an Option of the reference to the sender inside the
        // option, which is ready to be consumed with unwrap().send()...
        // This manuveur leaves the transmitter Option untouched which is required since
        // it is a field in the ThreadPool struct while still gaining access to the sender
        self.transmitter.as_ref().unwrap().send(job).unwrap();
    }
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        
        // Dropping the transmiter as the first part of the drop implementation of ThreadPool
        // s.t. the infinite loop in the spawned thread containing the receiver enter the Err
        // the break path s.t. they are deemed completed when join() is later called on the thread
        if let Some(transmitter) = self.transmitter.take() {
            core::mem::drop(transmitter);
        }

        // use pattern matching of for loop to fine tune
        // the binding spec of the worker variable/pattern after for
        // https://doc.rust-lang.org/book/ch18-01-all-the-places-for-patterns.html#for-loops
        for worker in &mut self.workers {

            println!("Shutting down worker {}", worker.id);

            // This manuveur is required since the thread is a field of the worker struct
            // thread.join() consumes the ownership of the thread, which is impossible if
            // thread is a direct field of a worker struct. Instead, by wrapping the thread
            // with an option, we gain ownership of the thread by using take() on the option
            // s.t. thread ownership is obtained and None is left inside the worker
            // https://doc.rust-lang.org/book/ch20-03-graceful-shutdown-and-cleanup.html#implementing-the-drop-trait-on-threadpool
            if let Some(thread) = worker.thread.take() {
                // join blocks the current (main) thread to wait for the associated thread to finish
                // https://doc.rust-lang.org/std/thread/struct.JoinHandle.html#method.join
                thread.join().unwrap();
            }
        }
    }
}