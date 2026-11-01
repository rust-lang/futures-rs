#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_mut)]
#![allow(unused_variables)]

use futures_core::future::{FusedFuture, Future};
use futures_core::task::{Context, Poll};
use slab::Slab;
use std::cell::UnsafeCell;
use std::fmt;
use std::mem;
use std::ops::{Deref, DerefMut};
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::task::Waker;

// Sentinel for when no slot in the `Slab` has been dedicated to this object.
const WAIT_KEY_NONE: usize = usize::max_value();

const PHASE: usize = 1;
const LOCKED: usize = 1 << 1;
const ONE_READER: usize = 1 << 2;
const READER_OFFSET: usize = 2;
const READER_OVERFLOW: usize = 1 << (usize::BITS - 1);

enum Ticket {
    Stale(usize),
    None,
}

enum Waiter {
    Waiting(Waker),
    Woken,
}

impl Waiter {
    fn register(&mut self, waker: &Waker) {
        match self {
            Waiter::Waiting(w) if waker.will_wake(w) => {}
            _ => *self = Waiter::Waiting(waker.clone()),
        }
    }

    fn wake(&mut self) -> bool {
        match mem::replace(self, Waiter::Woken) {
            Waiter::Waiting(waker) => waker.wake(),
            Waiter::Woken => return false,
        }

        true
    }
}

struct WaiterSet {
    waiters: Mutex<Slab<Waiter>>,
}

impl WaiterSet {
    fn new() -> Self {
        WaiterSet { waiters: Mutex::new(Slab::new()) }
    }

    fn register(&self, key: usize, waker: &Waker) -> usize {
        let mut this = self.waiters.lock().unwrap();

        if key == WAIT_KEY_NONE {
            this.insert(Waiter::Waiting(waker.clone()))
        } else {
            this[key].register(waker);
            key
        }
    }

    fn remove(&self, key: usize) {
        let mut this = self.waiters.lock().unwrap();

        if key != WAIT_KEY_NONE {
            this.remove(key);
        }
    }

    fn notify_all(&self) -> bool {
        let mut this = self.waiters.lock().unwrap();
        this.iter_mut().fold(false, |acc, (_, waiter)| waiter.wake() || acc)
    }

    fn notify_one(&self) -> bool {
        let mut this = self.waiters.lock().unwrap();
        this.iter_mut().any(|(_, waiter)| waiter.wake())
    }
}

/// A futures-aware read-write lock.
pub struct RwLock<T: ?Sized> {
    state: AtomicUsize,
    blocked: WaiterSet,
    writers: WaiterSet,
    value: UnsafeCell<T>,
}

impl<T: ?Sized> fmt::Debug for RwLock<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let state = self.state.load(Ordering::Relaxed);
        f.debug_struct("RwLock")
            .field("readers", &(state >> READER_OFFSET))
            .field("locked", &(state & LOCKED != 0))
            .field("phase", &(state & PHASE))
            .finish()
    }
}

impl<T> RwLock<T> {
    /// Creates a new futures-aware read-write lock.
    pub fn new(t: T) -> RwLock<T> {
        RwLock {
            state: AtomicUsize::new(0),
            blocked: WaiterSet::new(),
            writers: WaiterSet::new(),
            value: UnsafeCell::new(t),
        }
    }

    /// Consumes the read-write lock, returning the underlying data.
    pub fn into_inner(self) -> T {
        self.value.into_inner()
    }
}

impl<T: ?Sized> RwLock<T> {
    /// Acquire a read access lock asynchronously.
    ///
    /// This method returns a future that will resolve once all write access
    /// locks have been dropped.
    pub fn read(&self) -> RwLockReadFuture<'_, T> {
        RwLockReadFuture { rwlock: Some(self), key: WAIT_KEY_NONE, ticket: Ticket::None }
    }

    /// Acquire a write access lock asynchronously.
    ///
    /// This method returns a future that will resolve once all other locks
    /// have been dropped.
    pub fn write(&self) -> RwLockWriteFuture<'_, T> {
        RwLockWriteFuture { rwlock: Some(self), key: WAIT_KEY_NONE, ticket: Ticket::None }
    }

    /// Attempt to acquire a read access lock synchronously.
    pub fn try_read(&self) -> Option<RwLockReadGuard<'_, T>> {
        let ticket = self.state.fetch_add(ONE_READER, Ordering::Acquire) + ONE_READER;

        if ticket >= READER_OVERFLOW {
            panic!("reader count has overflowed");
        }

        if ticket & PHASE == 0 {
            return Some(RwLockReadGuard { rwlock: self });
        }

        match self.state.fetch_update(Ordering::Acquire, Ordering::Relaxed, |ticket| {
            if ticket & PHASE != 0 && ticket & LOCKED == 0 {
                Some(ticket ^ PHASE)
            } else {
                None
            }
        }) {
            Ok(_) => {
                self.blocked.notify_all();
                Some(RwLockReadGuard { rwlock: self })
            }
            Err(ticket) if ticket & PHASE == 0 => Some(RwLockReadGuard { rwlock: self }),
            Err(_) => {
                self.state
                    .fetch_update(Ordering::Release, Ordering::Relaxed, |ticket| {
                        if ticket & LOCKED != 0 && ticket >> READER_OFFSET == 1 {
                            Some((ticket - ONE_READER) | PHASE)
                        } else {
                            Some(ticket - ONE_READER)
                        }
                    })
                    .unwrap();

                self.blocked.notify_all();
                None
            }
        }
    }

    /// Attempt to acquire a write access lock synchronously.
    pub fn try_write(&self) -> Option<RwLockWriteGuard<'_, T>> {
        let ticket = self.state.fetch_or(LOCKED, Ordering::Acquire);

        if ticket & LOCKED != 0 {
            return None;
        }

        if ticket & PHASE != 0 {
            return Some(RwLockWriteGuard { rwlock: self });
        }

        match self.state.fetch_update(Ordering::Acquire, Ordering::Relaxed, |ticket| {
            if ticket & PHASE == 0 && ticket >> READER_OFFSET == 0 {
                Some(ticket | PHASE)
            } else {
                None
            }
        }) {
            Ok(_) => Some(RwLockWriteGuard { rwlock: self }),
            Err(ticket) if ticket & PHASE != 0 => Some(RwLockWriteGuard { rwlock: self }),
            Err(_) => {
                self.state
                    .fetch_update(Ordering::Release, Ordering::Relaxed, |ticket| {
                        Some(ticket ^ LOCKED)
                    })
                    .unwrap();

                self.writers.notify_one();
                self.blocked.notify_all();
                None
            }
        }
    }

    /// Returns a mutable reference to the underlying data.
    ///
    /// Since this call borrows the lock mutably, no actual locking needs to
    /// take place -- the mutable borrow statically guarantees no locks exist.
    pub fn get_mut(&mut self) -> &mut T {
        unsafe { &mut *self.value.get() }
    }
}

/// A future which resolves when the target read access lock has been successfully
/// acquired.
pub struct RwLockReadFuture<'a, T: ?Sized> {
    key: usize,
    ticket: Ticket,
    rwlock: Option<&'a RwLock<T>>,
}

impl<'a, T: ?Sized> Future for RwLockReadFuture<'a, T> {
    type Output = RwLockReadGuard<'a, T>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let rwlock = self.rwlock.expect("polled RwLockReadFuture after completion");

        let ticket = match self.ticket {
            Ticket::None => rwlock.state.fetch_add(ONE_READER, Ordering::Acquire) + ONE_READER,
            Ticket::Stale(_) => rwlock.state.load(Ordering::Acquire),
        };

        if ticket >= READER_OVERFLOW {
            panic!("reader count has overflowed");
        }

        if ticket & PHASE == 0 {
            self.rwlock = None;
            rwlock.blocked.remove(self.key);
            return Poll::Ready(RwLockReadGuard { rwlock });
        }

        let mut pending = false;

        loop {
            match rwlock.state.fetch_update(Ordering::Acquire, Ordering::Relaxed, |ticket| {
                if ticket & PHASE != 0 && ticket & LOCKED == 0 {
                    Some(ticket ^ PHASE)
                } else {
                    None
                }
            }) {
                Ok(_) => {
                    self.rwlock = None;
                    rwlock.blocked.remove(self.key);
                    rwlock.blocked.notify_all();
                    return Poll::Ready(RwLockReadGuard { rwlock });
                }
                Err(ticket) if ticket & PHASE == 0 => {
                    self.rwlock = None;
                    rwlock.blocked.remove(self.key);
                    return Poll::Ready(RwLockReadGuard { rwlock });
                }
                Err(ticket) => {
                    self.ticket = Ticket::Stale(ticket);

                    if pending {
                        return Poll::Pending;
                    } else {
                        self.key = rwlock.blocked.register(self.key, cx.waker());
                        pending = true;
                    }
                }
            }
        }
    }
}

impl<T: ?Sized> Drop for RwLockReadFuture<'_, T> {
    fn drop(&mut self) {
        if let Some(rwlock) = self.rwlock {
            rwlock.blocked.remove(self.key);

            match self.ticket {
                Ticket::None => {}
                Ticket::Stale(_) => {
                    rwlock
                        .state
                        .fetch_update(Ordering::Release, Ordering::Relaxed, |ticket| {
                            if ticket & LOCKED != 0 && ticket >> READER_OFFSET == 1 {
                                Some((ticket - ONE_READER) | PHASE)
                            } else {
                                Some(ticket - ONE_READER)
                            }
                        })
                        .unwrap();

                    rwlock.blocked.notify_all();
                }
            }
        }
    }
}

impl<T: ?Sized> FusedFuture for RwLockReadFuture<'_, T> {
    fn is_terminated(&self) -> bool {
        self.rwlock.is_none()
    }
}

impl<T: ?Sized> fmt::Debug for RwLockReadFuture<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RwLockReadFuture")
            .field("rwlock", &self.rwlock)
            .field("key", &self.key)
            .finish()
    }
}

/// An RAII guard returned by the `read` and `try_read` methods.
/// When all of these structures are dropped (fallen out of scope), the
/// rwlock will be available for write access.
pub struct RwLockReadGuard<'a, T: ?Sized> {
    rwlock: &'a RwLock<T>,
}

impl<T: ?Sized> Drop for RwLockReadGuard<'_, T> {
    fn drop(&mut self) {
        self.rwlock
            .state
            .fetch_update(Ordering::Release, Ordering::Relaxed, |ticket| {
                if ticket & LOCKED != 0 && ticket >> READER_OFFSET == 1 {
                    Some((ticket - ONE_READER) | PHASE)
                } else {
                    Some(ticket - ONE_READER)
                }
            })
            .unwrap();

        self.rwlock.blocked.notify_all();
    }
}

impl<T: ?Sized> Deref for RwLockReadGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &*self.rwlock.value.get() }
    }
}

impl<T: ?Sized + fmt::Debug> fmt::Debug for RwLockReadGuard<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RwLockReadGuard")
            .field("rwlock", &self.rwlock)
            .field("value", &&**self)
            .finish()
    }
}

/// A future which resolves when the target write access lock has been successfully
/// acquired.
pub struct RwLockWriteFuture<'a, T: ?Sized> {
    key: usize,
    ticket: Ticket,
    rwlock: Option<&'a RwLock<T>>,
}

impl<'a, T: ?Sized> Future for RwLockWriteFuture<'a, T> {
    type Output = RwLockWriteGuard<'a, T>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let rwlock = self.rwlock.expect("polled RwLockWriteFuture after completion");

        let ticket = match self.ticket {
            Ticket::None => {
                let mut pending = false;
                let mut ticket;

                loop {
                    ticket = rwlock.state.fetch_or(LOCKED, Ordering::Acquire);

                    if ticket & LOCKED == 0 {
                        break;
                    }

                    if pending {
                        return Poll::Pending;
                    }

                    self.key = rwlock.writers.register(self.key, cx.waker());
                    pending = true;
                }

                rwlock.writers.remove(self.key);
                self.key = WAIT_KEY_NONE;
                ticket
            }
            Ticket::Stale(_) => rwlock.state.load(Ordering::Acquire),
        };

        if ticket & PHASE != 0 {
            self.rwlock = None;
            rwlock.blocked.remove(self.key);
            return Poll::Ready(RwLockWriteGuard { rwlock });
        }

        let mut pending = false;

        loop {
            match rwlock.state.fetch_update(Ordering::Acquire, Ordering::Relaxed, |ticket| {
                if ticket & PHASE == 0 && ticket >> READER_OFFSET == 0 {
                    Some(ticket | PHASE)
                } else {
                    None
                }
            }) {
                Ok(_) => {
                    self.rwlock = None;
                    rwlock.blocked.remove(self.key);
                    return Poll::Ready(RwLockWriteGuard { rwlock });
                }
                Err(ticket) if ticket & PHASE != 0 => {
                    self.rwlock = None;
                    rwlock.blocked.remove(self.key);
                    return Poll::Ready(RwLockWriteGuard { rwlock });
                }
                Err(ticket) => {
                    self.ticket = Ticket::Stale(ticket);

                    if pending {
                        return Poll::Pending;
                    }

                    self.key = rwlock.blocked.register(self.key, cx.waker());
                    pending = true;
                }
            }
        }
    }
}

impl<T: ?Sized> Drop for RwLockWriteFuture<'_, T> {
    fn drop(&mut self) {
        if let Some(rwlock) = self.rwlock {
            match self.ticket {
                Ticket::None => {
                    rwlock.writers.remove(self.key);
                }
                Ticket::Stale(_) => {
                    rwlock.blocked.remove(self.key);

                    rwlock
                        .state
                        .fetch_update(Ordering::Release, Ordering::Relaxed, |ticket| {
                            Some(ticket ^ LOCKED)
                        })
                        .unwrap();

                    rwlock.writers.notify_one();
                    rwlock.blocked.notify_all();
                }
            }
        }
    }
}

impl<T: ?Sized> FusedFuture for RwLockWriteFuture<'_, T> {
    fn is_terminated(&self) -> bool {
        self.rwlock.is_none()
    }
}

impl<T: ?Sized> fmt::Debug for RwLockWriteFuture<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RwLockWriteFuture")
            .field("rwlock", &self.rwlock)
            .field("key", &self.key)
            .finish()
    }
}

/// An RAII guard returned by the `write` and `try_write` methods.
/// When this structure is dropped (falls out of scope), the rwlock
/// will be available for a future read or write access.
pub struct RwLockWriteGuard<'a, T: ?Sized> {
    rwlock: &'a RwLock<T>,
}

impl<T: ?Sized> Drop for RwLockWriteGuard<'_, T> {
    fn drop(&mut self) {
        let ticket = self
            .rwlock
            .state
            .fetch_update(Ordering::Release, Ordering::Relaxed, |ticket| {
                if ticket >> READER_OFFSET != 0 {
                    Some(ticket ^ LOCKED ^ PHASE)
                } else {
                    Some(ticket ^ LOCKED)
                }
            })
            .unwrap();

        self.rwlock.writers.notify_one();

        if ticket >> READER_OFFSET != 0 {
            self.rwlock.blocked.notify_all();
        }
    }
}

impl<T: ?Sized> Deref for RwLockWriteGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &*self.rwlock.value.get() }
    }
}

impl<T: ?Sized> DerefMut for RwLockWriteGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.rwlock.value.get() }
    }
}

impl<T: ?Sized + fmt::Debug> fmt::Debug for RwLockWriteGuard<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RwLockWriteGuard")
            .field("rwlock", &self.rwlock)
            .field("value", &&**self)
            .finish()
    }
}

unsafe impl<T: ?Sized + Send> Send for RwLock<T> {}
unsafe impl<T: ?Sized + Sync> Sync for RwLock<T> {}

unsafe impl<T: ?Sized + Send> Send for RwLockReadFuture<'_, T> {}
unsafe impl<T: ?Sized> Sync for RwLockReadFuture<'_, T> {}

unsafe impl<T: ?Sized + Send> Send for RwLockWriteFuture<'_, T> {}
unsafe impl<T: ?Sized> Sync for RwLockWriteFuture<'_, T> {}

unsafe impl<T: ?Sized + Send> Send for RwLockReadGuard<'_, T> {}
unsafe impl<T: ?Sized + Sync> Sync for RwLockReadGuard<'_, T> {}

unsafe impl<T: ?Sized + Send> Send for RwLockWriteGuard<'_, T> {}
unsafe impl<T: ?Sized + Sync> Sync for RwLockWriteGuard<'_, T> {}
