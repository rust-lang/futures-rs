//! TODO: dox

use std::prelude::v1::*;

use std::cell::{RefCell};
use std::rc::{Rc, Weak};

use futures_core::{Future, Poll, Async, Stream};
use futures_core::task::{Context, Waker, LocalMap};
use futures_core::executor::{Executor, SpawnError};
use futures_util::stream::FuturesUnordered;

use thread::ThreadNotify;
use enter;

struct Task {
    fut: Box<Future<Item = (), Error = ()>>,
    map: LocalMap,
}

/// todo: dox
pub struct LocalPool {
    pool: FuturesUnordered<Task>,
    incoming: Rc<Incoming>,
}

/// todo: dox
pub struct LocalExecutor {
    incoming: Weak<Incoming>,
}

type Incoming = RefCell<Vec<Task>>;

fn run_executor<T, F: FnMut(&Waker) -> Async<T>>(mut f: F) -> T {
    let _enter = enter()
        .expect("cannot execute `LocalPool` executor from within \
                 another executor");

    ThreadNotify::with_current(|thread| {
        let waker = &Waker::from(thread.clone());
        loop {
            if let Async::Ready(t) = f(waker) {
                return t;
            }
            thread.park();
        }
    })
}

impl LocalPool {
    /// todo: dox
    pub fn new() -> LocalPool {
        LocalPool {
            pool: FuturesUnordered::new(),
            incoming: Default::default(),
        }
    }

    /// todo: dox
    pub fn executor(&self) -> LocalExecutor {
        LocalExecutor {
            incoming: Rc::downgrade(&self.incoming)
        }
    }

    /// todo: dox
    pub fn run(&mut self, exec: &Executor) {
        run_executor(|waker| self.poll_pool(waker, exec))
    }

    /// todo: dox
    pub fn run_until<F, E>(&mut self, mut f: F, exec: &Executor) -> Result<F::Item, F::Error>
        where F: Future
    {
        // persistent state for the "main task"
        let mut main_map = LocalMap::new();

        run_executor(|waker| {
            let mut main_cx = Context::new(&mut main_map, waker, exec);

            // if our main task is done, so are we
            match f.poll(&mut main_cx) {
                Ok(Async::Ready(v)) => return Async::Ready(Ok(v)),
                Err(err) => return Async::Ready(Err(err)),
                _ => {}
            }

            self.poll_pool(waker, exec);
            Async::Pending
        })
    }

    // dox
    fn poll_pool(&mut self, waker: &Waker, exec: &Executor) -> Async<()> {
        // empty the incoming queue of newly-spawned tasks
        {
            let mut incoming = self.incoming.borrow_mut();
            for task in incoming.drain(..) {
                self.pool.push(task)
            }
        }

        // state for the FuturesUnordered, which will never be used
        let mut pool_map = LocalMap::new();
        let mut pool_cx = Context::new(&mut pool_map, waker, exec);

        loop {
            if let Ok(ret) = self.pool.poll(&mut pool_cx) {
                match ret {
                    Async::Pending => return Async::Pending,
                    Async::Ready(None) => return Async::Ready(()),
                    _ => {}
                }
            }
        }
    }
}

impl Executor for LocalExecutor {
    fn spawn(&self, f: Box<Future<Item = (), Error = ()> + Send>) -> Result<(), SpawnError> {
        self.spawn_task(Task {
            fut: f,
            map: LocalMap::new(),
        })
    }

    fn status(&self) -> Result<(), SpawnError> {
        if self.incoming.upgrade().is_some() {
            Ok(())
        } else {
            Err(SpawnError::shutdown())
        }
    }
}

impl LocalExecutor {
    fn spawn_task(&self, task: Task) -> Result<(), SpawnError> {
        let incoming = self.incoming.upgrade().ok_or(SpawnError::shutdown())?;
        incoming.borrow_mut().push(task);
        Ok(())
    }

    /// dox
    pub fn spawn_local<F>(&self, f: F) -> Result<(), SpawnError>
        where F: Future<Item = (), Error = ()> + 'static
    {
        self.spawn_task(Task {
            fut: Box::new(f),
            map: LocalMap::new(),
        })
    }
}

impl Future for Task {
    type Item = ();
    type Error = ();

    fn poll(&mut self, cx: &mut Context) -> Poll<(), ()> {
        self.fut.poll(&mut cx.with_locals(&mut self.map))
    }
}
