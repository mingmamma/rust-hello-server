// use std::{thread, sync::{mpsc::{self, Receiver}, Arc, Mutex}};
use std::thread::{self, JoinHandle};
use std::sync::{Arc, Mutex, mpsc::{channel, Sender, Receiver, RecvError}};

// Hypothetically correct type specification for a value that
// esscially is a closure of no parameter and returns unit AND 
// to be passable to other threads for execution. The compiler
// gives alternative format for actual use with same semantics
// type Job = Box<dyn FnOnce<(), Output = ()> + Send>;

type Job = Box<dyn FnOnce() -> () + Send>;

struct ThreadWorker {
    work_thread: Option<JoinHandle<()>>,
}

impl ThreadWorker {
    // an exhibit of pitfall that would fail compilation for violating the trait
    // bounds on parameters of the thread spawn method. Specifically, the
    // constraint of concern in this case requires the closure arguement to be Send
    
    // https://doc.rust-lang.org/std/thread/fn.spawn.html#
    // pub fn spawn<F, T>(f: F) -> JoinHandle<T>
    // where F: FnOnce() -> T + Send + 'static, T: ...
    
    // On the other hand, in the bogus implementation, the job_receiver is of
    // type Arc<Receiver<T>> as an argument to the new associated function.
    // Calling the recv() method in the closure code can be considered as
    // the closure instance holding (word choice?!) a value of type &Arc<Receiver<T>>,
    // which is due to the closure's capture by immutable reference of the
    // outer variable job_receiver of type Arc<Receiver<T>>, to be able to call 
    // the recv() method on the wrapped value of type Receiver<T> by taking
    // &Receiver<T>, made accessible by the Deref coersion enabled by Arc<T> implementing
    // the Deref trait, i.e. &Arc<Receiver<T> -> &Receiver<T> in this case
    // https://doc.rust-lang.org/std/sync/struct.Arc.html#impl-Deref-for-Arc%3CT,+A%3E
    // https://doc.rust-lang.org/std/sync/mpsc/struct.Receiver.html#method.recv
    
    // Given that Receiver<T> is not Sync, Arc<Receiver<T>> is not Send since Arc<U> is Send
    // only if U is Sync + Send (https://doc.rust-lang.org/std/sync/struct.Arc.html#impl-Send-for-Arc%3CT,+A%3E)
    // Given Arc<Receiver<T>> is not Send, &Arc<Receiver<T>> is not Sync, by definition
    // https://doc.rust-lang.org/std/marker/trait.Sync.html
    // Hence the closure instance holding a value of type &Receiver<T> is not Send
    // due to autotrait implementation (elaborate how?! if that is the case), 
    // contradicting the required trait bound that the closure argument is Send. U+220E

    // pub fn new(job_receiver: Arc<Receiver<()>>) -> Self {
    //     let handle = thread::spawn(|| {
    //         loop {
    //             println!("thread worker is just cruising like i am");
                
    //             if let Ok(_) = job_receiver.recv() {
    //                 println!("now i got job to do")
    //             } 
    //         }
    //     });
        
    //     Self {
    //         work_thread: handle,
    //     }
    // }
    
    pub fn new(job_receiver: Arc<Mutex<Receiver<Job>>>) -> Self {
        let handle = thread::spawn(move || {
            loop {
                // println!("thread worker is just cruising like i am");
                // an exhibit of a subtly problematic implementation of
                // acquiring the Mutex lock to receiver to receive a job
                // for execution. The performance issue with the code
                // is that the acquired lock guard is NOT dropped until
                // the job (essentially the passed in closure for execution)
                // completes running. It becomes a risk if the job is long-running,
                // preventing other potentially available threads to acquire lock
                // and obtain other jobs for execution in the meantime
                // if let Ok(job) = job_receiver.lock().unwrap().recv() {
                //     println!("now i got job to do");
                //     job();
                // }
                
                let job_msg: Result<Job, RecvError> = job_receiver.lock().unwrap().recv();
                match job_msg {
                    Ok(job) => {
                        job();
                    },
                    // the only circumstance that the recv() method returns Err variant
                    // is due to dropped sender, hence the workflow of the Drop implementation
                    // works by first dropping the sender, resulting in breaking out of the
                    // loop and completing the closure as parameter of the spawn() method,
                    // and lastly making the join() method call on the thread handle to return
                    Err(_) => {
                        break;
                    }
                }
            }
        });
        
        Self {
            work_thread: Some(handle),
        }            
    }
}

pub struct ThreadPool {
    job_sender: Option<Sender<Job>>,
    thread_workers: Vec<ThreadWorker>,
}

impl ThreadPool {
    pub fn build(thread_num: usize) -> Self
    {   
        // simple idiomatic prerequisite validation
        assert_ne!(thread_num, 0);
        
        let (job_sender, job_receiver) = channel::<Job>();
        
        let mut thread_workers: Vec<ThreadWorker> = Vec::with_capacity(thread_num);
        
        let job_receiver = Arc::new(Mutex::new(job_receiver));
        for _ in 0..thread_num {
            let job_receiver_copy: Arc<Mutex<Receiver<Job>>> = Arc::clone(&job_receiver);
            let new_thread_worker = ThreadWorker::new(job_receiver_copy);
            thread_workers.push(new_thread_worker);
        }
        
        Self {
            job_sender: Some(job_sender),
            thread_workers: thread_workers,
        }
        
    }
    
    // an example of incorrect signature design using Fn trait. Noting the
    // call site use, we see that the passed in closure arguement captures
    // an outer variable of type TcpStream, which is not copy, and hence would
    // involve taking the ownership of the captured variable by the effect of
    // the catpture. Such behaviour contradicts with the constraint of Fn trait,
    // whose class of closures can only capture outer variables by immutable
    // reference or not capturing any out variable at all
    // pub fn execute<F>(&self, f: F) -> () where F: Fn() -> ()
    
    // for what reason does the type parameter need to be explicitly static?!
    pub fn execute<F>(&self, f: F) -> () where F: FnOnce() -> () + Send + 'static
    {   
        let job = Box::new(f);
        self.job_sender.as_ref().unwrap().send(job).expect("failed to send job to worker thread for execution");
    }
}

// best-effort bogus Drop implementation fails due to calling JoinHandle<T>'s join() method
// on a JoinHandle<T> value being a field of the ThreadWorker struct. It violates borrow 
// borrow checking since at the point of the call, only a shared reference to the ThreadWorker
// instance is available. The Option wrapping can work around to move a field value out
// to allow the join() method call to take ownership and come through
// impl Drop for ThreadPool {
//     fn drop(&mut self) {
//         for thread_worker_ref in &self.thread_workers {
//             thread_worker_ref.work_thread.join().expect("failed to join work thread handle");
//         }
//     }
// }

impl Drop for ThreadPool {
    fn drop(&mut self) {
        drop(self.job_sender.take().unwrap());        
    
        for thread_worker_ref_mut in &mut self.thread_workers {
            let work_thread_handle = thread_worker_ref_mut.work_thread.take().unwrap();
            work_thread_handle.join().expect("failed to join worker thread");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    use rand::{self, Rng};
    use std::time::Duration;
    
    #[test]
    fn thread_pool_integration() {
        let test_counter = Arc::new(Mutex::new(0));
        {
            let test_thread_pool = ThreadPool::build(4);
            for _ in 0..5 {
                let test_counter_clone = Arc::clone(&test_counter);
                test_thread_pool.execute(move || {
                    let rnd_pause: u64 = rand::thread_rng().gen_range(999..1000);
                    thread::sleep(Duration::from_millis(rnd_pause));
                    
                    let mut counter_guard = test_counter_clone.lock().expect("failed to acquired Mutex lock");
                    *counter_guard = *counter_guard + 1;
                });            
            }
        }
    
        // hack measure to sleep to main thread to make sure the spawned worker threads in the thread pool
        // gets sufficient time to execute the inteeded jobs and not terminated prematurely as a result of the
        // completion of the main thread. It becomes unneccessary with properly implemented graceful shutdown
        // on ThreadPool, such that it is guaranteed that worker threads join after the completion on in-flight
        // jobs as part of the Drop implementation getting executed on the main thread when the ThreadPool instance 
        // goes out of scope, thus making sure that code after the scope of the ThreadPool "happens-after"
        // the all the spawned jobs in worker threads are completed
        // thread::sleep(Duration::from_secs(1));
        
        assert_eq!(*test_counter.lock().expect("failed to acquired Mutex lock"), 5);
    }
}