//! A simple crate for executing work on a thread pool, and getting back a
//! future.
//!
//! This crate provides a simple thread pool abstraction for running work
//! externally from the current thread that's running. An instance of `Future`
//! is handed back to represent that the work may be done later, and further
//! computations can be chained along with it as well.
//!
//! ```rust
//! extern crate futures;
//! extern crate futures_cpupool;
//!
//! use std::sync::mpsc::channel;
//!
//! use futures::{BoxFuture, Future};
//! use futures_cpupool::CpuPool;
//!
//! # fn long_running_future(a: u32) -> BoxFuture<u32, ()> { futures::done(Ok(a)).boxed() }
//! # fn main() {
//!
//! // Create a worker thread pool with four threads
//! let pool = CpuPool::new(4);
//!
//! // Execute some work on the thread pool, optionally closing over data.
//! let a = pool.spawn(long_running_future(2));
//! let b = pool.spawn(long_running_future(100));
//!
//! // Express some further computation once the work is completed on the thread
//! // pool.
//! let c = a.join(b).map(|(a, b)| a + b).wait().unwrap();
//!
//! // Print out the result
//! println!("{:?}", c);
//! # }
//! ```

#![deny(missing_docs)]

extern crate crossbeam;

#[macro_use]
extern crate futures;
extern crate num_cpus;

use std::panic::AssertUnwindSafe;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;

use crossbeam::sync::MsQueue;
use futures::{Future, oneshot, Oneshot, Complete, Poll};
use futures::task::{Task, Run, Executor};

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

struct Sender<F, T> {
    fut: F,
    tx: Option<Complete<T>>,
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
    size: usize,
}

/// The type of future returned from the `CpuPool::spawn` function, which
/// proxies the futures running on the thread pool.
///
/// This future will resolve in the same way as the underlying future, and it
/// will propagate panics.
pub struct CpuFuture<T, E> {
    inner: Oneshot<thread::Result<Result<T, E>>>,
}

enum Message {
    Run(Run),
    Close,
}

impl CpuPool {
    /// Creates a new thread pool with `size` worker threads associated with it.
    ///
    /// The returned handle can use `execute` to run work on this thread pool,
    /// and clones can be made of it to get multiple references to the same
    /// thread pool.
    pub fn new(size: usize) -> CpuPool {
        let pool = CpuPool {
            inner: Arc::new(Inner {
                queue: MsQueue::new(),
                cnt: AtomicUsize::new(1),
                size: size,
            }),
        };

        for _ in 0..size {
            let pool = CpuPool { inner: pool.inner.clone() };
            thread::spawn(|| pool.work());
        }

        return pool
    }

    /// Creates a new thread pool with a number of workers equal to the number
    /// of CPUs on the host.
    pub fn new_num_cpus() -> CpuPool {
        CpuPool::new(num_cpus::get())
    }

    /// Execute some work on this thread pool, returning a future to the work
    /// that's running on the thread pool.
    ///
    /// This function will execute the closure `f` on the associated thread
    /// pool, and return a future representing the finished computation. The
    /// future will either resolve to `R` if the computation finishes
    /// successfully or to `Box<Any+Send>` if it panics.
    pub fn spawn<F>(&self, f: F) -> CpuFuture<F::Item, F::Error>
        where F: Future + Send + 'static,
              F::Item: Send + 'static,
              F::Error: Send + 'static,
    {
        let (tx, rx) = oneshot();
        let sender = Sender {
            fut: AssertUnwindSafe(f).catch_unwind(),
            tx: Some(tx),
        };
        Task::new(self.inner.clone(), sender.boxed()).unpark();
        CpuFuture { inner: rx }
    }

    fn work(self) {
        let mut done = false;
        while !done {
            match self.inner.queue.pop() {
                Message::Close => done = true,
                Message::Run(r) => r.run(),
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
        if self.inner.cnt.fetch_sub(1, Ordering::Relaxed) > 1 {
            return
        }
        for _ in 0..self.inner.size {
            self.inner.queue.push(Message::Close);
        }
    }
}

impl Executor for Inner {
    fn execute(&self, run: Run) {
        self.queue.push(Message::Run(run))
    }
}

impl<T: Send + 'static, E: Send + 'static> Future for CpuFuture<T, E> {
    type Item = T;
    type Error = E;

    fn poll(&mut self) -> Poll<T, E> {
        match self.inner.poll() {
            Poll::Ok(res) => res.unwrap().into(),
            Poll::Err(_) => panic!("shouldn't be canceled"),
            Poll::NotReady => Poll::NotReady,
        }
    }
}

impl<F: Future> Future for Sender<F, Result<F::Item, F::Error>> {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Poll<(), ()> {
        if let Poll::Ok(_) = self.tx.as_mut().unwrap().poll_cancel() {
            // Cancelled, bail out
            return Poll::Ok(());
        }

        let res = try_poll!(self.fut.poll());
        self.tx.take().unwrap().complete(res);
        Poll::Ok(())
    }
}
