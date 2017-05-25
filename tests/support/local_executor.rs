//! Execution of futures on a single thread
//!
//! This module has no special handling of any blocking operations other than
//! futures-aware inter-thread communications, and should therefore probably not
//! be used to manage IO.

use std::sync::{Arc, Mutex, mpsc};
use std::collections::HashMap;
use std::collections::hash_map;
use std::boxed::Box;
use std::rc::Rc;
use std::cell::RefCell;

use futures::{Future, Async};

use futures::executor::{self, Spawn};

/// Main loop object
pub struct Core {
    unpark_send: mpsc::Sender<usize>,
    unpark: mpsc::Receiver<usize>,
    live: HashMap<usize, Spawn<Box<Future<Item=(), Error=()>>>>,
    next_id: usize,
}

impl Core {
    /// Create a new `Core`.
    pub fn new() -> Self {
        let (send, recv) = mpsc::channel();
        Core {
            unpark_send: send,
            unpark: recv,
            live: HashMap::new(),
            next_id: 0,
        }
    }

    /// Spawn a future to be executed by a future call to `run`.
    pub fn spawn<F>(&mut self, f: F)
        where F: Future<Item=(), Error=()> + 'static
    {
        self.live.insert(self.next_id, executor::spawn(Box::new(f)));
        self.unpark_send.send(self.next_id).unwrap();
        self.next_id += 1;
    }

    /// Run the loop until all futures previously passed to `spawn` complete.
    pub fn wait(&mut self) {
        while !self.live.is_empty() {
            self.turn();
        }
    }

    /// Run the loop until the future `f` completes.
    pub fn run<F>(&mut self, f: F) -> Result<F::Item, F::Error>
        where F: Future + 'static,
              F::Item: 'static,
              F::Error: 'static,
    {
        let out = Rc::new(RefCell::new(None));
        let out2 = out.clone();
        self.spawn(f.then(move |x| { *out.borrow_mut() = Some(x); Ok(()) }));
        loop {
            self.turn();
            if let Some(x) = out2.borrow_mut().take() {
                return x;
            }
        }
    }

    fn turn(&mut self) {
        let task = self.unpark.recv().unwrap(); // Safe to unwrap because self.unpark_send keeps the channel alive
        let notify = Arc::new(Notify { task: task, send: Mutex::new(self.unpark_send.clone()), });
        let mut task = if let hash_map::Entry::Occupied(x) = self.live.entry(task) { x } else { return };
        let result = task.get_mut().poll_future_notify(&notify, 0);
        match result {
            Ok(Async::Ready(())) => { task.remove(); }
            Err(()) => { task.remove(); }
            Ok(Async::NotReady) => {}
        }
    }
}

struct Notify {
    task: usize,
    send: Mutex<mpsc::Sender<usize>>,
}

impl executor::Notify for Notify {
    fn notify(&self, _id: usize) {
        let _ = self.send.lock().unwrap().send(self.task);
    }
}
