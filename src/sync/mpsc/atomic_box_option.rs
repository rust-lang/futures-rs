use std::boxed::Box;
use std::sync::atomic::AtomicPtr;
use std::sync::atomic::Ordering;
use std::ptr;
use std::mem;


/// Atomic holder of `Option<Box<T>>`.
///
/// `AtomicBoxOption` owns a pointer, thus it `drop` content on self `drop`.
#[derive(Debug)]
pub struct AtomicBoxOption<T>(AtomicPtr<T>);

impl<T> AtomicBoxOption<T> {
    /// Create an empty object with value `0`.
    #[inline]
    pub fn new() -> AtomicBoxOption<T> {
        AtomicBoxOption(AtomicPtr::new(ptr::null_mut()))
    }

    #[inline]
    pub fn load_is_some(&self, ordering: Ordering) -> bool {
        self.0.load(ordering) != ptr::null_mut()
    }

    #[inline]
    pub fn swap_null(&self, ordering: Ordering) -> Option<Box<T>> {
        let prev = self.0.swap(ptr::null_mut(), ordering);
        if prev != ptr::null_mut() {
            Some(unsafe { Box::from_raw(prev) })
        } else {
            None
        }
    }

    #[inline]
    pub fn swap_box(&self, b: Box<T>, ordering: Ordering) -> Option<Box<T>> {
        let prev = self.0.swap(b.as_ref() as *const T as *mut T, ordering);

        mem::forget(b);

        if prev != ptr::null_mut() {
            Some(unsafe { Box::from_raw(prev) })
        } else {
            None
        }
    }
}

impl<T> Drop for AtomicBoxOption<T> {
    fn drop(&mut self) {
        let p = self.0.load(Ordering::SeqCst);
        if p != ptr::null_mut() {
            unsafe { Box::from_raw(p); }
        }
    }
}


