use std::sync::atomic::{self, AtomicBool};
use std::cell::UnsafeCell;
use std::ops;
use std::marker::Sync;
use Future;

/// A mutually-exclusive container.
///
/// This type provides access to the inner value, such that only one thread can access it at a
/// time. This way thread-safety is upheld.
///
/// Contrary to the classical mutex, this mutex is based on futures, meaning that you can
/// "asynchronously lock" the mutex.
#[derive(Debug)]
pub struct Mutex<T> {
    locked: AtomicBool,
    data: UnsafeCell<T>,
}

impl<T> Mutex<T> {
    /// Create a new mutex with some initial value.
    pub fn new(init: T) -> Mutex<T> {
        Mutex {
            locked: AtomicBool::new(false),
            data: UnsafeCell::new(init),
        }
    }

    /// Create a future to lock this mutex.
    ///
    /// The future completes with a RAII guard for the inner value when the lock is acquired. Keep
    /// in mind that this is lazy and will first attempt to acquire the lock when run. That means
    /// that this method itself does nothing, but constructs a future, which shall then be run for
    /// actual effect.
    #[inline]
    pub fn lock(&self) -> MutexFuture<T> {
        MutexFuture {
            mutex: self,
        }
    }
}

unsafe impl<T> Sync for Mutex<T> {}

/// Future for a pending mutex lock.
#[derive(Debug)]
pub struct MutexFuture<'a, T: 'a> {
    mutex: &'a Mutex<T>,
}

impl<'a, T> Future for MutexFuture<'a, T> {
    type Item = MutexGuard<'a, T>;
    type Error = ();

    fn poll(&mut self) -> ::Poll<MutexGuard<'a, T>, ()> {
        if self.mutex.locked.swap(true, atomic::Ordering::Relaxed) {
            Ok(::Async::NotReady)
        } else {
            Ok(::Async::Ready(MutexGuard {
                mutex: self.mutex,
            }))
        }
    }
}

/// An RAII guard for a `Mutex` lock.
#[derive(Debug)]
pub struct MutexGuard<'a, T: 'a> {
    mutex: &'a Mutex<T>,
}

impl<'a, T> ops::Deref for MutexGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &*self.mutex.data.get() }
    }
}

impl<'a, T> ops::DerefMut for MutexGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.mutex.data.get() }
    }
}

impl<'a, T> Drop for MutexGuard<'a, T> {
    fn drop(&mut self) {
        self.mutex.locked.store(false, atomic::Ordering::Relaxed);
    }
}
