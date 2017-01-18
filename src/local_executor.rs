//! Execution of futures on a single thread
//!
//! This module has no special handling of any blocking operations other than
//! futures-aware inter-thread communications, and should therefore probably not
//! be used to manage IO.

use std::sync::{Arc, Mutex, mpsc};
use std::collections::HashMap;
use std::collections::hash_map;
use std::boxed::Box;

use {Future, Async};

use executor::{self, Spawn};

/// Main loop object
pub struct Core {
    unpark_send: mpsc::Sender<u64>,
    unpark: mpsc::Receiver<u64>,
    live: HashMap<u64, Spawn<Box<Future<Item=(), Error=()>>>>,
    next_id: u64,
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
    ///
    /// # Panics
    /// May, but is not guaranteed to, panic if a future that was passed to `spawn` deadlocks.
    pub fn run(&mut self) {
        while !self.live.is_empty() {
            if let Ok(task) = self.unpark.recv() {
                let unpark = Arc::new(Unpark { task: task, send: Mutex::new(self.unpark_send.clone()), });
                let mut task = if let hash_map::Entry::Occupied(x) = self.live.entry(task) { x } else { continue };
                let result = task.get_mut().poll_future(unpark);
                match result {
                    Ok(Async::Ready(())) => { task.remove_entry(); }
                    Err(()) => { task.remove_entry(); }
                    Ok(Async::NotReady) => {}
                }
            } else {
                panic!("deadlock: {} task(s) blocked", self.live.len());
            }
        }
    }
}

struct Unpark {
    task: u64,
    send: Mutex<mpsc::Sender<u64>>,
}

impl executor::Unpark for Unpark {
    fn unpark(&self) {
        let _ = self.send.lock().unwrap().send(self.task);
    }
}
