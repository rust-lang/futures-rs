use std::cell::UnsafeCell;
use std::ops::{Deref, DerefMut};
use std::sync::atomic::Ordering::{Acquire, Release};
use std::sync::atomic::AtomicBool;

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

    pub fn borrow(&self) -> Option<Borrow<T>> {
        if !self.in_use.swap(true, Acquire) {
            Some(Borrow { __ptr: self })
        } else {
            None
        }
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

#[cfg(test)]
mod tests {
    use super::AtomicCell;

    #[test]
    fn smoke() {
        let a = AtomicCell::new(1);
        let mut a1 = a.borrow().unwrap();
        assert!(a.borrow().is_none());
        assert_eq!(*a1, 1);
        *a1 = 2;
        drop(a1);
        assert_eq!(*a.borrow().unwrap(), 2);
        assert_eq!(*a.borrow().unwrap(), 2);
    }
}
