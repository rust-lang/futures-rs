//! Futures-powered synchronization primitives.

use alloc::sync::Arc;
use core::cell::UnsafeCell;
use core::fmt;
#[cfg(feature = "bilock")]
use core::mem;
use core::ops::{Deref, DerefMut};
use core::pin::Pin;
#[cfg(feature = "bilock")]
use core::ptr;
use core::sync::atomic::AtomicU8;
use core::sync::atomic::Ordering::{AcqRel, Acquire, Relaxed};
#[cfg(feature = "bilock")]
use futures_core::future::Future;
use futures_core::task::{Context, Poll, Waker};

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
///
/// This type is only available when the `bilock` feature of this
/// library is activated.
#[cfg_attr(docsrs, doc(cfg(feature = "bilock")))]
pub struct BiLock<T> {
    arc: Arc<Inner<T>>,
    token: u8,
    left: bool,
}

impl<T> fmt::Debug for BiLock<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BiLock").field("arc", &self.arc).field("token", &self.token).finish()
    }
}

struct Inner<T> {
    token: AtomicU8,
    waker: UnsafeCell<Option<(bool, Waker)>>,
    value: Option<UnsafeCell<T>>,
}

impl<T> fmt::Debug for Inner<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Inner").field("token", &self.token).field("waker", &self.waker).finish()
    }
}

// Lock/Unlock implementation
//
// There are 3 tokens,
const TOKEN_LOCK: u8 = 2;
const TOKEN_WAKE: u8 = 1;
const TOKEN_NULL: u8 = 0;
// TOKEN_LOCK starts in the inner lock's token field, with the other two tokens starting in the two
// bilock halves
//
// Possession of TOKEN_LOCK gives exclusive access to the inner value
// Possession of TOKEN_WAKE gives exclusive access to the waker field
//
// To poll the lock:
// We atomically swap our held token with the token in inner
// if we receive TOKEN_LOCK, the lock was successful, return Poll::Ready
// if we receive TOKEN_NULL, we swap our token again
// if we receive TOKEN_WAKE, we store our waker in the waker field, then swap our token again,
// On the second swap:
// if we receive TOKEN_LOCK, the lock was successful, return Poll::Ready
// If we receive TOKEN_NULL, we must have stored our token after the first swap, return Poll::Pending
// If we receive TOKEN_WAKE, we store our waker in the waker field, then swap our token again,
// On the third swap:
// If we receive TOKEN_LOCK, the lock was successful, return Poll::Ready
// If we receive TOKEN_NULL, we stored our token after the second swap, return Poll::Pending
//
// To unlock the lock:
// We swap our token (which must be TOKEN_LOCK) with the inner token:
// if we receive TOKEN_WAKE: wake the waker (if present)
// if we receive TOKEN_NULL: there is no contention, do nothing
//
// In addition to the protocol described above we implement the following optimizations:
// On lock:
// Store an identifier representing our half of the bilock alongside the waker
// If the existing waker will_wake our waker, do not replace it
// On unlock:
// If the stored waker belongs to our half, do not wake it

unsafe impl<T: Send> Send for Inner<T> {}
unsafe impl<T: Send> Sync for Inner<T> {}

impl<T> BiLock<T> {
    /// Creates a new `BiLock` protecting the provided data.
    ///
    /// Two handles to the lock are returned, and these are the only two handles
    /// that will ever be available to the lock. These can then be sent to separate
    /// tasks to be managed there.
    ///
    /// The data behind the bilock is considered to be pinned, which allows `Pin`
    /// references to locked data. However, this means that the locked value
    /// will only be available through `Pin<&mut T>` (not `&mut T`) unless `T` is `Unpin`.
    /// Similarly, reuniting the lock and extracting the inner value is only
    /// possible when `T` is `Unpin`.
    pub fn new(t: T) -> (Self, Self) {
        let arc = Arc::new(Inner {
            token: AtomicU8::new(TOKEN_LOCK),
            value: Some(UnsafeCell::new(t)),
            waker: UnsafeCell::new(None),
        });

        (
            Self { arc: arc.clone(), token: TOKEN_WAKE, left: true },
            Self { arc, token: TOKEN_NULL, left: false },
        )
    }

    /// Attempt to acquire this lock, returning `Pending` if it can't be
    /// acquired.
    ///
    /// This function will acquire the lock in a nonblocking fashion, returning
    /// immediately if the lock is already held. If the lock is successfully
    /// acquired then `Poll::Ready` is returned with a value that represents
    /// the locked value (and can be used to access the protected data). The
    /// lock is unlocked when the returned `BiLockGuard` is dropped.
    ///
    /// If the lock is already held then this function will return
    /// `Poll::Pending`. In this case the current task will also be scheduled
    /// to receive a notification when the lock would otherwise become
    /// available.
    ///
    /// # Panics
    ///
    /// This function will panic if called outside the context of a future's
    /// task.
    pub fn poll_lock(&mut self, cx: &mut Context<'_>) -> Poll<BiLockGuard<'_, T>> {
        assert_ne!(self.token, TOKEN_LOCK);
        if self.token == TOKEN_WAKE {
            if let Ok(token) =
                self.arc.token.compare_exchange_weak(TOKEN_LOCK, self.token, AcqRel, Relaxed)
            {
                self.token = token;
                return Poll::Ready(BiLockGuard { bilock: self });
            }
        } else {
            self.token = self.arc.token.swap(self.token, Acquire);
            if self.token == TOKEN_LOCK {
                return Poll::Ready(BiLockGuard { bilock: self });
            }
        }
        // token must be TOKEN_WAKE here
        assert_eq!(self.token, TOKEN_WAKE);
        {
            let our_waker = cx.waker();
            // SAFETY: we own the wake token, so we have exclusive access to this field.
            // The mutable reference we create goes out of scope before we give up the
            // token
            match unsafe { &mut *self.arc.waker.get() } {
                waker @ None => *waker = Some((self.left, our_waker.clone())),
                Some((left, waker)) => {
                    if !our_waker.will_wake(waker) {
                        *left = self.left;
                        *waker = our_waker.clone()
                    }
                }
            }
        }
        // Maybe we spin here a few times?

        for _ in 0..5 {
            match self.arc.token.compare_exchange_weak(TOKEN_LOCK, self.token, AcqRel, Relaxed) {
                Ok(token) => {
                    self.token = token;
                    return Poll::Ready(BiLockGuard { bilock: self });
                }
                Err(_) => {}
            }
            std::hint::spin_loop();
        }
        self.token = self.arc.token.swap(self.token, AcqRel);
        match self.token {
            TOKEN_LOCK => Poll::Ready(BiLockGuard { bilock: self }),
            TOKEN_NULL => Poll::Pending,
            _ => unreachable!(),
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
    #[cfg(feature = "bilock")]
    #[cfg_attr(docsrs, doc(cfg(feature = "bilock")))]
    pub fn lock(self) -> BiLockAcquire<T> {
        BiLockAcquire { bilock: Some(self) }
    }

    /// Attempts to put the two "halves" of a `BiLock<T>` back together and
    /// recover the original value. Succeeds only if the two `BiLock<T>`s
    /// originated from the same call to `BiLock::new`.
    pub fn reunite(self, other: Self) -> Result<T, ReuniteError<T>>
    where
        T: Unpin,
    {
        if Arc::ptr_eq(&self.arc, &other.arc) {
            drop(other);
            let inner = Arc::try_unwrap(self.arc)
                .expect("futures: try_unwrap failed in BiLock<T>::reunite");
            Ok(unsafe { inner.into_value() })
        } else {
            Err(ReuniteError(self, other))
        }
    }

    fn unlock(&mut self) {
        assert_eq!(self.token, TOKEN_LOCK);
        self.token = self.arc.token.swap(self.token, AcqRel);
        match self.token {
            TOKEN_NULL => {} // lock uncontended
            TOKEN_WAKE => {
                // SAFETY: we own the wake token, so we have exclusive access to this field.
                if let Some((left, wake)) = unsafe { &mut *self.arc.waker.get() } {
                    if self.left != *left {
                        // don't wake our own waker
                        wake.wake_by_ref()
                    }
                }
            }
            _ => {
                unreachable!()
            }
        }
    }
}

impl<T: Unpin> Inner<T> {
    unsafe fn into_value(mut self) -> T {
        self.value.take().unwrap().into_inner()
    }
}

impl<T> Drop for Inner<T> {
    fn drop(&mut self) {
        assert_eq!(*self.token.get_mut(), TOKEN_LOCK);
    }
}

/// Error indicating two `BiLock<T>`s were not two halves of a whole, and
/// thus could not be `reunite`d.
#[cfg_attr(docsrs, doc(cfg(feature = "bilock")))]
pub struct ReuniteError<T>(pub BiLock<T>, pub BiLock<T>);

impl<T> fmt::Debug for ReuniteError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("ReuniteError").field(&"...").finish()
    }
}

impl<T> fmt::Display for ReuniteError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "tried to reunite two BiLocks that don't form a pair")
    }
}

#[cfg(feature = "std")]
impl<T: core::any::Any> std::error::Error for ReuniteError<T> {}

/// Returned RAII guard from the `poll_lock` method.
///
/// This structure acts as a sentinel to the data in the `BiLock<T>` itself,
/// implementing `Deref` and `DerefMut` to `T`. When dropped, the lock will be
/// unlocked.
#[derive(Debug)]
#[cfg_attr(docsrs, doc(cfg(feature = "bilock")))]
pub struct BiLockGuard<'a, T> {
    bilock: &'a mut BiLock<T>,
}

impl<T> Deref for BiLockGuard<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        unsafe { &*self.bilock.arc.value.as_ref().unwrap().get() }
    }
}

impl<T: Unpin> DerefMut for BiLockGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.bilock.arc.value.as_ref().unwrap().get() }
    }
}

impl<T> BiLockGuard<'_, T> {
    /// Get a mutable pinned reference to the locked value.
    pub fn as_pin_mut(&mut self) -> Pin<&mut T> {
        // Safety: we never allow moving a !Unpin value out of a bilock, nor
        // allow mutable access to it
        unsafe { Pin::new_unchecked(&mut *self.bilock.arc.value.as_ref().unwrap().get()) }
    }
}

impl<T> Drop for BiLockGuard<'_, T> {
    fn drop(&mut self) {
        self.bilock.unlock();
    }
}

/// Resolved value of the `BiLockAcquire<T>` future.
///
/// This value, like `BiLockGuard<T>`, is a sentinel to the value `T` through
/// implementations of `Deref` and `DerefMut`. When dropped will unlock the
/// lock, and the original unlocked `BiLock<T>` can be recovered through the
/// `unlock` method.
#[derive(Debug)]
#[cfg(feature = "bilock")]
pub struct BiLockAcquired<T> {
    bilock: BiLock<T>,
}

#[cfg(feature = "bilock")]
impl<T> Deref for BiLockAcquired<T> {
    type Target = T;
    fn deref(&self) -> &T {
        unsafe { &*self.bilock.arc.value.as_ref().unwrap().get() }
    }
}

#[cfg(feature = "bilock")]
impl<T: Unpin> DerefMut for BiLockAcquired<T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.bilock.arc.value.as_ref().unwrap().get() }
    }
}

#[cfg(feature = "bilock")]
impl<T> BiLockAcquired<T> {
    /// Get a mutable pinned reference to the locked value.
    pub fn as_pin_mut(&mut self) -> Pin<&mut T> {
        // Safety: we never allow moving a !Unpin value out of a bilock, nor
        // allow mutable access to it
        unsafe { Pin::new_unchecked(&mut *self.bilock.arc.value.as_ref().unwrap().get()) }
    }
    /// Recovers the original `BiLock<T>`, unlocking this lock.
    pub fn unlock(self) -> BiLock<T> {
        let mut bilock = unsafe { ptr::read(&self.bilock) }; // get the lock out without running our destructor
        mem::forget(self);
        mem::drop(BiLockGuard { bilock: &mut bilock }); // unlock the lock
        bilock
    }
}

#[cfg(feature = "bilock")]
impl<T> Drop for BiLockAcquired<T> {
    fn drop(&mut self) {
        self.bilock.unlock();
    }
}

/// Future returned by `BiLock::lock` which will resolve when the lock is
/// acquired.
#[cfg(feature = "bilock")]
#[cfg_attr(docsrs, doc(cfg(feature = "bilock")))]
#[must_use = "futures do nothing unless you `.await` or poll them"]
#[derive(Debug)]
pub struct BiLockAcquire<T> {
    bilock: Option<BiLock<T>>,
}

// Pinning is never projected to fields
#[cfg(feature = "bilock")]
impl<T> Unpin for BiLockAcquire<T> {}

#[cfg(feature = "bilock")]
impl<T> Future for BiLockAcquire<T> {
    type Output = BiLockAcquired<T>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let bilock = self.bilock.as_mut().expect("Cannot poll after Ready");
        match bilock.poll_lock(cx) {
            Poll::Pending => return Poll::Pending,
            Poll::Ready(guard) => {
                mem::forget(guard); // don't run the destructor, so the lock stays locked by us
            }
        }
        Poll::Ready(BiLockAcquired { bilock: self.bilock.take().unwrap() })
    }
}
