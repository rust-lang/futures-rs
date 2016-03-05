use std::cell::UnsafeCell;
use std::mem;
use std::ops::{Deref, DerefMut};
use std::sync::atomic::Ordering::{self, Acquire, Release, SeqCst};
use std::sync::atomic::{AtomicBool, AtomicUsize};

pub struct AtomicCell<T> {
    in_use: AtomicBool,
    data: UnsafeCell<T>,
}

pub struct Borrow<'a, T: 'a> {
    __ptr: &'a AtomicCell<T>,
}

unsafe impl<T: Send> Send for AtomicCell<T> {}
unsafe impl<T: Send> Sync for AtomicCell<T> {}

impl<T> AtomicCell<T> {
    pub fn new(t: T) -> AtomicCell<T> {
        AtomicCell {
            in_use: AtomicBool::new(false),
            data: UnsafeCell::new(t),
        }
    }

    pub fn replace(&self, data: T) -> Result<T, T> {
        if let Some(mut borrow) = self.borrow() {
            Ok(mem::replace(&mut *borrow, data))
        } else {
            Err(data)
        }
    }

    pub fn borrow(&self) -> Option<Borrow<T>> {
        if !self.in_use.swap(true, Acquire) {
            Some(Borrow { __ptr: self })
        } else {
            None
        }
    }
}

impl<T: Copy> AtomicCell<T> {
    pub fn get(&self) -> Option<T> {
        self.borrow().map(|b| *b)
    }
}

impl<'a, T> Deref for Borrow<'a, T> {
    type Target = T;
    fn deref(&self) -> &T {
        unsafe { &*self.__ptr.data.get() }
    }
}

impl<'a, T> DerefMut for Borrow<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.__ptr.data.get() }
    }
}

impl<'a, T> Drop for Borrow<'a, T> {
    fn drop(&mut self) {
        self.__ptr.in_use.store(false, Release);
    }
}

pub struct AtomicCounter<T> {
    cnt: AtomicUsize,
    data: AtomicCell<Option<T>>,
}

impl<T> AtomicCounter<T> {
    pub fn new(data: T, cnt: usize) -> AtomicCounter<T> {
        AtomicCounter {
            cnt: AtomicUsize::new(cnt),
            data: AtomicCell::new(Some(data)),
        }
    }

    pub fn try_take(&self) -> Option<T> {
        if self.cnt.fetch_sub(1, SeqCst) == 1 {
            match self.data.replace(None) {
                Ok(data) => {
                    assert!(data.is_some());
                    data
                }
                Err(..) => panic!(),
            }
        } else {
            None
        }
    }
}

pub struct AtomicFlags {
    flags: AtomicUsize,
}

impl AtomicFlags {
    pub fn new(init: usize) -> AtomicFlags {
        AtomicFlags { flags: AtomicUsize::new(init) }
    }

    pub fn load(&self, ordering: Ordering) -> usize {
        self.flags.load(ordering)
    }

    pub fn modify<F>(&self, init: usize, ordering: Ordering, mut f: F) -> usize
        where F: FnMut(usize) -> usize
    {
        let mut cur = init;
        loop {
            let old = self.flags.compare_and_swap(cur, f(cur), ordering);
            if old == cur {
                return old
            }
            cur = old
        }
    }
}

#[cfg(test)]
mod tests {
    use super::AtomicCell;

    #[test]
    fn smoke() {
        let a = AtomicCell::new(1);
        assert_eq!(a.replace(2), Ok(1));
        assert_eq!(a.get(), Some(2));
        assert_eq!(a.replace(3), Ok(2));
    }
}
