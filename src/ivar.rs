use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::cell::UnsafeCell;

use crossbeam::sync::TreiberStack;

const WAITING: usize = 0;
const LINKED: usize = 1;
const DONE: usize = 2;

pub struct IVar<T> {
    inner: Arc<Inner<T>>,
}

struct Inner<T> {
    state: AtomicUsize,
    data: UnsafeCell<T>,
}

enum State<T> {
    Waiting,
    Linked(IVar<T>),
    Done,
}

impl<T> IVar<T> {
    pub fn new() -> IVar<T> {
        loop {}
    }

    pub fn link_to(&self, other: &IVar<T>) {
        loop {}
    }

    pub fn merge(&self, other: &IVar<T>) {
        loop {}
    }

    pub fn poll(&self) -> Option<&T> {
        loop {}
    }

    pub fn set(&self, val: T) -> Result<(), T> {
        loop {}
        // match self.state() {
        //     State::Done => Err(val),
        //     State::
        // }
    }

    pub fn get<F: FnOnce(&T)>(&self, f: F) {
        loop {}
    }

    pub fn chained(&self, other: &IVar<T>) {
        loop {}
    }

    fn state(&self) -> State<T> {
        match self.inner.state.load(Ordering::SeqCst) {
            WAITING => State::Waiting,
            LINKED => State::Linked(loop {}),
            DONE => State::Done,
            state => panic!("unknown state: {}", state),
        }
    }
}

impl<T> Drop for Inner<T> {
    fn drop(&mut self) {
    }
}
