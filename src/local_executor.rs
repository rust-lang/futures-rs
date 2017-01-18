//! Execution of futures which perform no IO on a single thread

use std::rc::Rc;
use std::sync::Arc;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::boxed::Box;

use {Future, Async};

use executor::{self, Spawn};

/// Main loop object
pub struct Core {
    inner: Rc<RefCell<Inner>>,
    live: usize,
}

impl Core {
    /// Create a new `Core`.
    pub fn new() -> Self {
        Core {
            inner: Rc::new(RefCell::new(Inner::new())),
            live: 0,
        }
    }

    /// Spawn a future to be executed by a future call to `run`.
    pub fn spawn<F>(&mut self, f: F)
        where F: Future<Item=(), Error=()> + 'static
    {
        self.inner.borrow_mut().ready.push_back(Rc::new(RefCell::new(executor::spawn(Box::new(f)))));
        self.live += 1;
    }

    /// Run the loop until all futures complete.
    ///
    /// # Panics
    /// Panics if a future deadlocks.
    pub fn run(&mut self) {
        loop {
            let task = self.inner.borrow_mut().ready.pop_front();
            if let Some(task) = task {
                let unpark = Arc::new(Unpark { task: task.clone(), inner: self.inner.clone(), });
                match task.borrow_mut().poll_future(unpark) {
                    Ok(Async::Ready(())) => self.live -= 1,
                    Err(()) => self.live -= 1,
                    Ok(Async::NotReady) => {}
                }
            } else {
                break;
            }
        }
        if self.live != 0 {
            panic!("deadlock");
        }
    }
}

struct Inner {
    ready: VecDeque<Rc<RefCell<Spawn<Box<Future<Item=(), Error=()>>>>>>,
}

impl Inner {
    fn new() -> Self {
        Inner {
            ready: VecDeque::new(),
        }
    }
}

struct Unpark {
    task: Rc<RefCell<Spawn<Box<Future<Item=(), Error=()>>>>>,
    inner: Rc<RefCell<Inner>>,
}

// We will do neither of these things.
unsafe impl ::std::marker::Sync for Unpark {}
unsafe impl ::std::marker::Send for Unpark {}

impl executor::Unpark for Unpark {
    fn unpark(&self) {
        self.inner.borrow_mut().ready.push_back(self.task.clone())
    }
}
