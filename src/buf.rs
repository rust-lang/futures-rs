use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicUsize, Ordering};

pub struct AtomicBuf<T> {
    next_push: AtomicUsize,
    amt: AtomicUsize,
    next_pop: AtomicUsize,
    slots: UnsafeCell<Vec<Option<T>>>,
}

unsafe impl<T: Send> Send for AtomicBuf<T> {}
unsafe impl<T: Send> Sync for AtomicBuf<T> {}

impl<T: Send> AtomicBuf<T> {
    pub fn new(capacity: usize) -> AtomicBuf<T> {
        AtomicBuf {
            next_push: AtomicUsize::new(0),
            amt: AtomicUsize::new(0),
            next_pop: AtomicUsize::new(0),
            slots: UnsafeCell::new((0..capacity).map(|_| None).collect()),
        }
    }

    pub fn push(&self, t: T) {
        let idx = self.next_push.fetch_add(1, Ordering::SeqCst);
        unsafe {
            let slots = self.slots.get();
            assert!(idx < (*slots).len());
            (*slots)[idx] = Some(t);
        }
        self.amt.fetch_add(1, Ordering::SeqCst);
    }

    pub fn pop(&self) -> Option<T> {
        let mut amt = self.amt.load(Ordering::SeqCst);
        loop {
            if amt == 0 {
                return None
            }
            let old = self.amt.compare_and_swap(amt, amt - 1, Ordering::SeqCst);
            if old == amt {
                break
            }
            amt = old;
        }
        let to_pop = self.next_pop.fetch_add(1, Ordering::SeqCst);
        unsafe {
            Some((*self.slots.get())[to_pop].take().unwrap())
        }
    }
}
