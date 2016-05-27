use std::cell::UnsafeCell;
use std::mem;
use std::sync::atomic::{AtomicUsize, Ordering};

pub struct AtomicOption<T> {
    // 0 == None
    // 1 == Some
    // 2 == reading or writing
    state: AtomicUsize,
    data: UnsafeCell<Option<T>>,
}

unsafe impl<T: Send> Sync for AtomicOption<T> {}
unsafe impl<T: Send> Send for AtomicOption<T> {}

impl<T> AtomicOption<T> {
    pub fn new(t: T) -> AtomicOption<T> {
        AtomicOption {
            state: AtomicUsize::new(1),
            data: UnsafeCell::new(Some(t)),
        }
    }

    pub fn take(&self) -> Option<T> {
        if self.state.compare_and_swap(1, 2, Ordering::SeqCst) != 1 {
            return None
        }
        unsafe {
            let ret = (*self.data.get()).take();
            assert!(ret.is_some());
            assert_eq!(self.state.swap(0, Ordering::SeqCst), 2);
            return ret
        }
    }

    pub fn put(&self, t: T) {
        if self.state.compare_and_swap(0, 2, Ordering::SeqCst) != 0 {
            panic!("can't put, it's in use or full");
        }
        unsafe {
            let prev = mem::replace(&mut *self.data.get(), Some(t));
            assert!(prev.is_none());
            assert_eq!(self.state.swap(1, Ordering::SeqCst), 2);
        }
    }
}
