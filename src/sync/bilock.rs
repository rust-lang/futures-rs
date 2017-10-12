use std::any::Any;
use std::cell::UnsafeCell;
use std::error::Error;
use std::fmt;
use std::mem;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering::{AcqRel, Relaxed};

use {Async, Future, Poll};
use task::{self, Task};

/// A type of futures-powered synchronization primitive which is a mutex between
/// two possible owners.
///
/// This primitive is not as generic as a full-blown mutex but is sufficient for
/// many use cases where there are only two possible owners of a resource. The
/// implementation of `BiLock` can be more optimized for just the two possible
/// owners.
///
/// Note that it's possible to use this lock through a poll-style interface with
/// the `poll_lock` method but you can also use it as a future with the `lock`
/// method that consumes a `BiLock` and returns a future that will resolve when
/// it's locked.
///
/// A `BiLock` is typically used for "split" operations where data which serves
/// two purposes wants to be split into two to be worked with separately. For
/// example a TCP stream could be both a reader and a writer or a framing layer
/// could be both a stream and a sink for messages. A `BiLock` enables splitting
/// these two and then using each independently in a futures-powered fashion.
#[derive(Debug)]
pub struct BiLock<T> {
    inner: Arc<Inner<T>>,
    // The `index` is set to `true` for one side of the `BiLock` and `false` for
    // the other side.
    index: bool
}

/// IMPLEMENTATION DETAILS
/// ======================
/// 
/// Token system
/// ------------
/// It is necessary to protect the unsynchronised parts of the data structure
/// (tasks, value) from concurrent access by both sides of the BiLock.
/// To achieve this, there are three "tokens" numbered 0, 1, 2. At any given
/// time, each side of the BiLock owns one of these tokens, and the remaining
/// token is free. To access an element in the task array or the value, the
/// corresponding token must be owned:
///  ___________________
/// | TOKEN |   FIELD   |
/// |_______|___________|
/// |     0 |  tasks[0] |
/// |     1 |  tasks[1] |
/// |     2 |   value   |
/// |_______|___________|
/// 
/// If one side of the BiLock owns the token "2", then it has taken the lock.
/// If the token "2" is free, then neither side has taken the lock.
/// 
/// State encoding
/// --------------
/// The ownership of the tokens is encoded entirely withing the state field,
/// which allows it to be updated atomically.
/// 
/// The "LEFT" and "RIGHT" columns indicate which token is owned by each side
/// of the BiLock, and the "FREE" column shows the remaining token:
///  _________________________________
/// | state % 6 | LEFT | FREE | RIGHT |
/// |___________|______|______|_______|
/// |         0 |    0 |    1 |     2 |
/// |         1 |    1 |    0 |     2 |
/// |         2 |    2 |    0 |     1 |
/// |         3 |    0 |    2 |     1 |
/// |         4 |    1 |    2 |     0 |
/// |         5 |    2 |    1 |     0 |
/// |___________|______|______|_______|
/// 
/// Each side of the BiLock can perform a single atomic operation: it can swap
/// its token with the free token.
/// 
/// For the LEFT side, by examining the table above, we can see that this
/// corresponds to the operation "XOR 1":
///     0 <=> 1
///     2 <=> 3
///     4 <=> 5
/// 
/// For the RIGHT side, by the same process we can see that this corresponds
/// to the operation "ADD 3 (mod 6)":
///     0 <=> 3
///     1 <=> 4
///     2 <=> 5
/// 
/// This yields a simple implementation of the swap operation for each side
/// of the BiLock: `state.fetch_xor(1, AcqRel)`, `state.fetch_add(3, AcqRel)`.
/// 
/// The "lock" operation
/// --------------------
/// To implement the "lock" operation, we must own token "0" or "1", or else we
/// would already have the lock. This gives us access to an entry in the
/// "tasks" array. We store the current task handle here in case we fail
/// to take the lock and need to be woken up.
/// 
/// Once we have stored the task handle, we perform the atomic swap, and
/// we check what token we got back. If we got back "2" then we succeeded
/// in taking the lock. Otherwise, we return NotReady, suspending the task.
/// If this occurs, the "FREE" token now contains our task handle.
/// 
/// The "unlock" operation
/// ----------------------
/// To implement the "unlock" operation, we must own token "2". We perform
/// the atomic swap operation, which results in the "FREE" token being "2"
/// (ie. the lock is not held by anyone). In exchange, we get back either
/// the "0" or "1" tokens.
/// 
/// At this point we must wake up the other side of the BiLock if it's
/// currently waiting on the lock. Conveniently, the token we just got
/// back gives us access to any task handle that was stored if the other
/// side suspended itself.
/// 
/// Optimisations
/// -------------
/// Obtaining the current task handle can be expensive, so the "lock"
/// implementation has a fast path which tries to take the lock without
/// storing a task handle. If this fails it simply tries to take the lock
/// again, this time storing a task handle. In the event that it fails that
/// second time, it returns NotReady.
/// 
/// Wrapping behaviour
/// ------------------
/// Since modular arithmatic is not a supported atomic operation, normal
/// addition is used in the "ADD 3 (mod 6)" operation. This results in the
/// `state` growing indefinitely. As a result, it must be protected from
/// wrapping around as it would naturally at a power of 2 determined by
/// the machine word size. After performing a lock, the side of the BiLock
/// which performs this addition checks if state is getting close to this
/// limit, and if so subtracts a large multiple of 6 to bring it back down
/// again.
#[derive(Debug)]
struct Inner<T> {
    // (state % 6) identifies a state in a state machine
    state: AtomicUsize,
    // Up to two tasks may be trying to park themselves concurrently
    tasks: [UnsafeCell<Option<Task>>; 2],
    // The protected value
    value: Option<UnsafeCell<T>>,
}

unsafe impl<T: Send> Send for Inner<T> {}
unsafe impl<T: Send> Sync for Inner<T> {}

impl<T> BiLock<T> {
    /// Creates a new `BiLock` protecting the provided data.
    ///
    /// Two handles to the lock are returned, and these are the only two handles
    /// that will ever be available to the lock. These can then be sent to separate
    /// tasks to be managed there.
    pub fn new(t: T) -> (BiLock<T>, BiLock<T>) {
        let inner = Arc::new(Inner {
            state: AtomicUsize::new(3),
            tasks: Default::default(),
            value: Some(UnsafeCell::new(t)),
        });

        (BiLock { inner: inner.clone(), index: false }, BiLock { inner: inner, index: true })
    }

    /// Attempt to acquire this lock, returning `NotReady` if it can't be
    /// acquired.
    ///
    /// This function will acquire the lock in a nonblocking fashion, returning
    /// immediately if the lock is already held. If the lock is successfully
    /// acquired then `Async::Ready` is returned with a value that represents
    /// the locked value (and can be used to access the protected data). The
    /// lock is unlocked when the returned `BiLockGuard` is dropped.
    ///
    /// # Panics
    ///
    /// This function will panic if called outside the context of a future's
    /// task, or if the lock is already held.
    pub fn poll_lock(&mut self) -> Async<BiLockGuard<T>> {
        if unsafe { self.inner.poll_lock(self.index) } {
            Async::Ready(BiLockGuard { inner: self })
        } else {
            Async::NotReady
        }
    }

    /// Perform a "blocking lock" of this lock, consuming this lock handle and
    /// returning a future to the acquired lock.
    ///
    /// This function consumes the `BiLock<T>` and returns a sentinel future,
    /// `BiLockAcquire<T>`. The returned future will resolve to
    /// `BiLockAcquired<T>` which represents a locked lock similarly to
    /// `BiLockGuard<T>`.
    ///
    /// Note that the returned future will never resolve to an error.
    pub fn lock(self) -> BiLockAcquire<T> {
        BiLockAcquire {
            inner: Some(self),
        }
    }

    /// Attempts to put the two "halves" of a `BiLock<T>` back together and
    /// recover the original value. Succeeds only if the two `BiLock<T>`s
    /// originated from the same call to `BiLock::new`.
    pub fn reunite(self, other: Self) -> Result<T, ReuniteError<T>> {
        if &*self.inner as *const _ == &*other.inner as *const _ {
            drop(other);
            let inner = Arc::try_unwrap(self.inner)
                .ok()
                .expect("futures: try_unwrap failed in BiLock<T>::reunite");
            Ok(unsafe { inner.into_inner() })
        } else {
            Err(ReuniteError(self, other))
        }
    }

    fn unlock(&mut self) {
        unsafe {
            self.inner.unlock(self.index);
        }
    }
}

impl<T> Inner<T> {
    unsafe fn into_inner(mut self) -> T {
        self.value.take().unwrap().into_inner()
    }

    unsafe fn poll_lock(&self, caller_index: bool) -> bool {
        let mut prev_state;
        if caller_index {
            // Swap our token with the "FREE" token
            prev_state = self.state.fetch_add(3, AcqRel);

            // See what happened
            match prev_state % 6 {
                // Slow paths if lock failed
                2 => {
                    // Store our task handle
                    *self.tasks[0].get() = Some(task::current());
                    // Try again
                    prev_state = self.state.fetch_add(3, AcqRel);
                },
                5 => {
                    // Store our task handle
                    *self.tasks[1].get() = Some(task::current());
                    // Try again
                    prev_state = self.state.fetch_add(3, AcqRel);
                },
                // If we had the lock originally, we should never have been called!
                0 | 1 => panic!("Lock already held"),
                // We obtained the lock!
                _ => {}
            }

            // Make sure the state wraps around at a multiple of 6, and leave plenty of
            // room at the end.
            const UPPER_LIMIT: usize = !0usize - 63;
            if prev_state >= UPPER_LIMIT {
                self.state.fetch_sub(UPPER_LIMIT, Relaxed);
            }
        } else {
            // Swap our token with the "FREE" token
            prev_state = self.state.fetch_xor(1, AcqRel);

            // See what happened
            match prev_state % 6 {
                // Slow paths if lock failed
                0 => {
                    // Store our task handle
                    *self.tasks[1].get() = Some(task::current());
                    // Try again
                    prev_state = self.state.fetch_xor(1, AcqRel);
                },
                1 => {
                    // Store our task handle
                    *self.tasks[0].get() = Some(task::current());
                    // Try again
                    prev_state = self.state.fetch_xor(1, AcqRel);
                },
                // If we had the lock originally, we should never have been called!
                2 | 5 => panic!("Lock already held"),
                // We obtained the lock!
                _ => {}
            }
        }

        // Return true if we obtained the lock
        match prev_state % 6 {
            3 | 4 => true,
            _ => false
        }
    }

    unsafe fn unlock(&self, caller_index: bool) {
        if caller_index {
            // Swap our token with the "FREE" token
            let prev_state = self.state.fetch_add(3, AcqRel);

            // Wake up a waiting task if necessary
            (*match prev_state % 6 {
                0 => self.tasks[1].get(),
                1 => self.tasks[0].get(),
                _ => panic!("Lock not held")
            }).take().map(|t| t.notify());
        } else {
            // Swap our token with the "FREE" token
            let prev_state = self.state.fetch_xor(1, AcqRel);

            // Wake up a waiting task if necessary
            (*match prev_state % 6 {
                2 => self.tasks[0].get(),
                5 => self.tasks[1].get(),
                _ => panic!("Lock not held")
            }).take().map(|t| t.notify());
        }
    }

    unsafe fn get_value(&self) -> &mut T {
        &mut *self.value.as_ref().unwrap().get()
    }
}

impl<T> Drop for Inner<T> {
    fn drop(&mut self) {
        assert!((*self.state.get_mut() + 1) % 6 > 3);
    }
}

/// Error indicating two `BiLock<T>`s were not two halves of a whole, and
/// thus could not be `reunite`d.
pub struct ReuniteError<T>(pub BiLock<T>, pub BiLock<T>);

impl<T> fmt::Debug for ReuniteError<T> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_tuple("ReuniteError")
            .field(&"...")
            .finish()
    }
}

impl<T> fmt::Display for ReuniteError<T> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "tried to reunite two BiLocks that don't form a pair")
    }
}

impl<T: Any> Error for ReuniteError<T> {
    fn description(&self) -> &str {
        "tried to reunite two BiLocks that don't form a pair"
    }
}

/// Returned RAII guard from the `poll_lock` method.
///
/// This structure acts as a sentinel to the data in the `BiLock<T>` itself,
/// implementing `Deref` and `DerefMut` to `T`. When dropped, the lock will be
/// unlocked.
#[derive(Debug)]
pub struct BiLockGuard<'a, T: 'a> {
    inner: &'a mut BiLock<T>,
}

impl<'a, T> Deref for BiLockGuard<'a, T> {
    type Target = T;
    fn deref(&self) -> &T {
        unsafe { self.inner.inner.get_value() }
    }
}

impl<'a, T> DerefMut for BiLockGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { self.inner.inner.get_value() }
    }
}

impl<'a, T> Drop for BiLockGuard<'a, T> {
    fn drop(&mut self) {
        self.inner.unlock();
    }
}

/// Future returned by `BiLock::lock` which will resolve when the lock is
/// acquired.
#[derive(Debug)]
pub struct BiLockAcquire<T> {
    inner: Option<BiLock<T>>,
}

impl<T> Future for BiLockAcquire<T> {
    type Item = BiLockAcquired<T>;
    type Error = ();

    fn poll(&mut self) -> Poll<BiLockAcquired<T>, ()> {
        match self.inner.as_mut().expect("cannot poll after Ready").poll_lock() {
            Async::Ready(r) => {
                mem::forget(r);
            }
            Async::NotReady => return Ok(Async::NotReady),
        }
        Ok(Async::Ready(BiLockAcquired {
            inner: self.inner.take()
        }))
    }
}

/// Resolved value of the `BiLockAcquire<T>` future.
///
/// This value, like `BiLockGuard<T>`, is a sentinel to the value `T` through
/// implementations of `Deref` and `DerefMut`. When dropped will unlock the
/// lock, and the original unlocked `BiLock<T>` can be recovered through the
/// `unlock` method.
#[derive(Debug)]
pub struct BiLockAcquired<T> {
    inner: Option<BiLock<T>>,
}

impl<T> BiLockAcquired<T> {
    /// Recovers the original `BiLock<T>`, unlocking this lock.
    pub fn unlock(mut self) -> BiLock<T> {
        let mut bi_lock = self.inner.take().unwrap();
        bi_lock.unlock();
        bi_lock
    }
}

impl<T> Deref for BiLockAcquired<T> {
    type Target = T;
    fn deref(&self) -> &T {
        unsafe { self.inner.as_ref().unwrap().inner.get_value() }
    }
}

impl<T> DerefMut for BiLockAcquired<T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { self.inner.as_ref().unwrap().inner.get_value() }
    }
}

impl<T> Drop for BiLockAcquired<T> {
    fn drop(&mut self) {
        if let Some(ref mut bi_lock) = self.inner {
            bi_lock.unlock();
        }
    }
}
