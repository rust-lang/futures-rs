//! A simple crate for executing work on a thread pool, and getting back a
//! future.
//!
//! This crate provides a simple thread pool abstraction for running work
//! externally from the current thread that's running. An instance of `Future`
//! is handed back to represent that the work may be done later, and futher
//! computations can be chained along with it as well.
//!
//! ```rust
//! extern crate futures;
//! extern crate futures_cpupool;
//!
//! use std::sync::mpsc::channel;
//!
//! use futures::Future;
//! use futures_cpupool::CpuPool;
//!
//! # fn long_running_computation() -> u32 { 2 }
//! # fn long_running_computation2(a: u32) -> u32 { a }
//! # fn main() {
//!
//! // Create a worker thread pool with four threads
//! let pool = CpuPool::new(4);
//!
//! // Execute some work on the thread pool, optionally closing over data.
//! let a = pool.execute(long_running_computation);
//! let b = pool.execute(|| long_running_computation2(100));
//!
//! // Express some further computation once the work is completed on the thread
//! // pool.
//! let c = a.join(b).map(|(a, b)| a + b);
//!
//! // Block the current thread to get the result.
//! let (tx, rx) = channel();
//! c.then(move |res| {
//!     tx.send(res)
//! }).forget();
//! let res = rx.recv().unwrap();
//!
//! // Print out the result
//! println!("{:?}", res);
//! # }
//! ```

#![deny(missing_docs)]

extern crate crossbeam;
extern crate futures;
extern crate num_cpus;

use std::any::Any;
use std::panic::{self, AssertUnwindSafe};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;

use crossbeam::sync::MsQueue;
use futures::{Future, promise, Promise, Task, Poll};

/// A thread pool intended to run CPU intensive work.
///
/// This thread pool will hand out futures representing the completed work
/// that happens on the thread pool itself, and the futures can then be later
/// composed with other work as part of an overall computation.
///
/// The worker threads associated with a thread pool are kept alive so long as
/// there is an open handle to the `CpuPool` or there is work running on them. Once
/// all work has been drained and all references have gone away the worker
/// threads will be shut down.
///
/// Currently `CpuPool` implements `Clone` which just clones a new reference to
/// the underlying thread pool.
pub struct CpuPool {
    inner: Arc<Inner>,
}

fn _assert() {
    fn _assert_send<T: Send>() {}
    fn _assert_sync<T: Sync>() {}
    _assert_send::<CpuPool>();
    _assert_sync::<CpuPool>();
}

struct Inner {
    queue: MsQueue<Message>,
    cnt: AtomicUsize,
    size: u32,
}

/// The type of future returned from the `CpuPool::execute` function.
///
/// This future will either resolve to `R`, the completed value, or
/// `Box<Any+Send>` if the computation panics (with the payload of the panic).
pub struct CpuFuture<R: Send + 'static> {
    inner: Promise<thread::Result<R>>,
}

trait Thunk: Send + 'static {
    fn call_box(self: Box<Self>);
}

impl<F: FnOnce() + Send + 'static> Thunk for F {
    fn call_box(self: Box<Self>) {
        (*self)()
    }
}

enum Message {
    Run(Box<Thunk>),
    Close,
}

impl CpuPool {
    /// Creates a new thread pool with `size` worker threads associated with it.
    ///
    /// The returned handle can use `execute` to run work on this thread pool,
    /// and clones can be made of it to get multiple references to the same
    /// thread pool.
    pub fn new(size: u32) -> CpuPool {
        let pool = CpuPool {
            inner: Arc::new(Inner {
                queue: MsQueue::new(),
                cnt: AtomicUsize::new(1),
                size: size,
            }),
        };

        for _ in 0..size {
            let pool = pool.clone();
            thread::spawn(|| pool.work());
        }

        return pool
    }

    /// Creates a new thread pool with a number of workers equal to the number
    /// of CPUs on the host.
    pub fn new_num_cpus() -> CpuPool {
        CpuPool::new(num_cpus::get() as u32)
    }

    /// Execute some work on this thread pool, returning a future to the work
    /// that's running on the thread pool.
    ///
    /// This function will execute the closure `f` on the associated thread
    /// pool, and return a future representing the finished computation. The
    /// future will either resolve to `R` if the computation finishes
    /// successfully or to `Box<Any+Send>` if it panics.
    pub fn execute<F, R>(&self, f: F) -> CpuFuture<R>
        where F: FnOnce() -> R + Send + 'static,
              R: Send + 'static,
    {
        let (tx, rx) = promise();
        self.inner.queue.push(Message::Run(Box::new(|| {
            tx.complete(panic::catch_unwind(AssertUnwindSafe(f)));
        })));
        CpuFuture { inner: rx }
    }

    fn work(self) {
        let mut done = false;
        while !done {
            match self.inner.queue.pop() {
                Message::Close => done = true,
                Message::Run(r) => r.call_box(),
            }
        }
    }
}

impl Clone for CpuPool {
    fn clone(&self) -> CpuPool {
        self.inner.cnt.fetch_add(1, Ordering::Relaxed);
        CpuPool { inner: self.inner.clone() }
    }
}

impl Drop for CpuPool {
    fn drop(&mut self) {
        if self.inner.cnt.fetch_sub(1, Ordering::Relaxed) != 0 {
            return
        }
        for _ in 0..self.inner.size {
            self.inner.queue.push(Message::Close);
        }
    }
}

impl<R: Send + 'static> Future for CpuFuture<R> {
    type Item = R;
    type Error = Box<Any + Send>;

    fn poll(&mut self, task: &mut Task) -> Poll<R, Box<Any + Send>> {
        match self.inner.poll(task) {
            Poll::Ok(res) => res.into(),
            Poll::Err(_) => panic!("shouldn't be canceled"),
            Poll::NotReady => Poll::NotReady,
        }
    }

    fn schedule(&mut self, task: &mut Task) {
        self.inner.schedule(task)
    }
}
