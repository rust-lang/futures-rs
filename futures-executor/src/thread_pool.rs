use std::prelude::v1::*;

use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::thread;
use std::fmt;

use futures_core::*;
use futures_core::task::{self, Wake, Waker, LocalMap};
use futures_core::executor::{Executor, SpawnError};
use futures_core::never::Never;

use enter;
use num_cpus;
use unpark_mutex::UnparkMutex;

/// A general-purpose thread pool for scheduling asynchronous tasks.
///
/// The thread pool multiplexes any number of tasks onto a fixed number of
/// worker threads.
pub struct ThreadPool {
    state: Arc<PoolState>,
}

/// Thread pool configuration object.
pub struct ThreadPoolBuilder {
    pool_size: usize,
    stack_size: usize,
    name_prefix: Option<String>,
    after_start: Option<Arc<Fn(usize) + Send + Sync>>,
    before_stop: Option<Arc<Fn(usize) + Send + Sync>>,
}

trait AssertSendSync: Send + Sync {}
impl AssertSendSync for ThreadPool {}

struct PoolState {
    tx: Mutex<mpsc::Sender<Message>>,
    rx: Mutex<mpsc::Receiver<Message>>,
    cnt: AtomicUsize,
    size: usize,
}

impl fmt::Debug for ThreadPool {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("ThreadPool")
            .field("size", &self.state.size)
            .finish()
    }
}

impl fmt::Debug for ThreadPoolBuilder {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("ThreadPoolBuilder")
            .field("pool_size", &self.pool_size)
            .field("name_prefix", &self.name_prefix)
            .finish()
    }
}

enum Message {
    Run(Task),
    Close,
}

impl ThreadPool {
    /// Creates a new thread pool with the default configuration.
    ///
    /// See documentation for the methods in
    /// [`ThreadPoolBuilder`](::ThreadPoolBuilder) for details on the default
    /// configuration.
    pub fn new() -> ThreadPool {
        ThreadPoolBuilder::new().create()
    }

    /// Create a default thread pool configuration, which can then be customized.
    ///
    /// See documentation for the methods in
    /// [`ThreadPoolBuilder`](::ThreadPoolBuilder) for details on the default
    /// configuration.
    pub fn builder() -> ThreadPoolBuilder {
        ThreadPoolBuilder::new()
    }

    /// Runs the given future with this thread pool as the default executor for
    /// spawning tasks.
    ///
    /// **This function will block the calling thread** until the given future
    /// is complete. While executing that future, any tasks spawned onto the
    /// default executor will be routed to this thread pool.
    ///
    /// Note that the function will return when the provided future completes,
    /// even if some of the tasks it spawned are still running.
    pub fn run<F: Future>(&mut self, f: F) -> Result<F::Item, F::Error> {
        ::LocalPool::new().run_until(f, self)
    }
}

impl Executor for ThreadPool {
    fn spawn(&mut self, f: Box<Future<Item = (), Error = Never> + Send>) -> Result<(), SpawnError> {
        let task = Task {
            spawn: f,
            map: LocalMap::new(),
            wake_handle: Arc::new(WakeHandle {
                exec: self.clone(),
                mutex: UnparkMutex::new(),
            }),
            exec: self.clone(),
        };
        self.state.send(Message::Run(task));
        Ok(())
    }
}

impl PoolState {
    fn send(&self, msg: Message) {
        self.tx.lock().unwrap().send(msg).unwrap();
    }

    fn work(&self,
            idx: usize,
            after_start: Option<Arc<Fn(usize) + Send + Sync>>,
            before_stop: Option<Arc<Fn(usize) + Send + Sync>>) {
        let _scope = enter().unwrap();
        after_start.map(|fun| fun(idx));
        loop {
            let msg = self.rx.lock().unwrap().recv().unwrap();
            match msg {
                Message::Run(r) => r.run(),
                Message::Close => break,
            }
        }
        before_stop.map(|fun| fun(idx));
    }
}

impl Clone for ThreadPool {
    fn clone(&self) -> ThreadPool {
        self.state.cnt.fetch_add(1, Ordering::Relaxed);
        ThreadPool { state: self.state.clone() }
    }
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        if self.state.cnt.fetch_sub(1, Ordering::Relaxed) == 1 {
            for _ in 0..self.state.size {
                self.state.send(Message::Close);
            }
        }
    }
}

impl ThreadPoolBuilder {
    /// Create a default thread pool configuration.
    ///
    /// See the other methods on this type for details on the defaults.
    pub fn new() -> ThreadPoolBuilder {
        ThreadPoolBuilder {
            pool_size: num_cpus::get(),
            stack_size: 0,
            name_prefix: None,
            after_start: None,
            before_stop: None,
        }
    }

    /// Set size of a future ThreadPool
    ///
    /// The size of a thread pool is the number of worker threads spawned.  By
    /// default, this is equal to the number of CPU cores.
    pub fn pool_size(&mut self, size: usize) -> &mut Self {
        self.pool_size = size;
        self
    }

    /// Set stack size of threads in the pool.
    ///
    /// By default, worker threads use Rust's standard stack size.
    pub fn stack_size(&mut self, stack_size: usize) -> &mut Self {
        self.stack_size = stack_size;
        self
    }

    /// Set thread name prefix of a future ThreadPool.
    ///
    /// Thread name prefix is used for generating thread names. For example, if prefix is
    /// `my-pool-`, then threads in the pool will get names like `my-pool-1` etc.
    ///
    /// By default, worker threads are assigned Rust's standard thread name.
    pub fn name_prefix<S: Into<String>>(&mut self, name_prefix: S) -> &mut Self {
        self.name_prefix = Some(name_prefix.into());
        self
    }

    /// Execute the closure `f` immediately after each worker thread is started,
    /// but before running any tasks on it.
    ///
    /// This hook is intended for bookkeeping and monitoring.
    /// The closure `f` will be dropped after the `builder` is dropped
    /// and all worker threads in the pool have executed it.
    ///
    /// The closure provided will receive an index corresponding to the worker
    /// thread it's running on.
    pub fn after_start<F>(&mut self, f: F) -> &mut Self
        where F: Fn(usize) + Send + Sync + 'static
    {
        self.after_start = Some(Arc::new(f));
        self
    }

    /// Execute closure `f` just prior to shutting down each worker thread.
    ///
    /// This hook is intended for bookkeeping and monitoring.
    /// The closure `f` will be dropped after the `builder` is droppped
    /// and all threads in the pool have executed it.
    ///
    /// The closure provided will receive an index corresponding to the worker
    /// thread it's running on.
    pub fn before_stop<F>(&mut self, f: F) -> &mut Self
        where F: Fn(usize) + Send + Sync + 'static
    {
        self.before_stop = Some(Arc::new(f));
        self
    }

    /// Create a [`ThreadPool`](::ThreadPool) with the given configuration.
    ///
    /// # Panics
    ///
    /// Panics if `pool_size == 0`.
    pub fn create(&mut self) -> ThreadPool {
        let (tx, rx) = mpsc::channel();
        let pool = ThreadPool {
            state: Arc::new(PoolState {
                tx: Mutex::new(tx),
                rx: Mutex::new(rx),
                cnt: AtomicUsize::new(1),
                size: self.pool_size,
            }),
        };
        assert!(self.pool_size > 0);

        for counter in 0..self.pool_size {
            let state = pool.state.clone();
            let after_start = self.after_start.clone();
            let before_stop = self.before_stop.clone();
            let mut thread_builder = thread::Builder::new();
            if let Some(ref name_prefix) = self.name_prefix {
                thread_builder = thread_builder.name(format!("{}{}", name_prefix, counter));
            }
            if self.stack_size > 0 {
                thread_builder = thread_builder.stack_size(self.stack_size);
            }
            thread_builder.spawn(move || state.work(counter, after_start, before_stop)).unwrap();
        }
        return pool
    }
}

/// Units of work submitted to an `Executor`, currently only created
/// internally.
struct Task {
    spawn: Box<Future<Item = (), Error = Never> + Send>,
    map: LocalMap,
    exec: ThreadPool,
    wake_handle: Arc<WakeHandle>,
}

struct WakeHandle {
    mutex: UnparkMutex<Task>,
    exec: ThreadPool,
}

impl Task {
    /// Actually run the task (invoking `poll` on its future) on the current
    /// thread.
    pub fn run(self) {
        let Task { mut spawn, wake_handle, mut map, mut exec } = self;
        let waker = Waker::from(wake_handle.clone());

        // SAFETY: the ownership of this `Task` object is evidence that
        // we are in the `POLLING`/`REPOLL` state for the mutex.
        unsafe {
            wake_handle.mutex.start_poll();

            loop {
                let res = {
                    let mut cx = task::Context::new(&mut map, &waker, &mut exec);
                    spawn.poll(&mut cx)
                };
                match res {
                    Ok(Async::Pending) => {}
                    Ok(Async::Ready(())) => return wake_handle.mutex.complete(),
                    Err(never) => match never {},
                }
                let task = Task {
                    spawn,
                    map,
                    wake_handle: wake_handle.clone(),
                    exec: exec
                };
                match wake_handle.mutex.wait(task) {
                    Ok(()) => return,            // we've waited
                    Err(r) => { // someone's notified us
                        spawn = r.spawn;
                        map = r.map;
                        exec = r.exec;
                    }
                }
            }
        }
    }
}

impl fmt::Debug for Task {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Task")
            .field("contents", &"...")
            .finish()
    }
}

impl Wake for WakeHandle {
    fn wake(arc_self: &Arc<Self>) {
        match arc_self.mutex.notify() {
            Ok(task) => arc_self.exec.state.send(Message::Run(task)),
            Err(()) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn test_drop_after_start() {
        let (tx, rx) = mpsc::sync_channel(2);
        let _cpu_pool = ThreadPoolBuilder::new()
            .pool_size(2)
            .after_start(move |_| tx.send(1).unwrap()).create();

        // After ThreadPoolBuilder is deconstructed, the tx should be droped
        // so that we can use rx as an iterator.
        let count = rx.into_iter().count();
        assert_eq!(count, 2);
    }
}
