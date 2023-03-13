use crate::task::{waker_ref, ArcWake};
use crate::wakerset::{WakerKey, WakerSet};
use alloc::fmt;
use alloc::sync::Arc;
use core::cell::UnsafeCell;
use core::marker::Unpin;
use core::pin::Pin;
use core::sync::atomic::AtomicUsize;
use core::sync::atomic::Ordering::{AcqRel, Acquire, Relaxed, Release};
use futures_core::stream::{FusedStream, Stream};
use futures_core::task::{Context, Poll};

/// A stream that is cloneable and can be polled in multiple threads.
/// Use the [`shared`](crate::StreamExt::shared) combinator method to convert
/// any stream into a `Shared` sream.
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct Shared<S: Stream> {
    /// Current index into the buffer
    idx: usize,
    waker_key: WakerKey,
    inner: Arc<Inner<S>>,
}

struct Slot<T> {
    /// This contains both the number of receivers on the current slot,
    /// as well as if the slot is full.
    /// The least significant bit is the full state, the rest is the refcount
    state: AtomicUsize,
    val: UnsafeCell<Option<T>>,
}

const REFCOUNT_MASK: usize = usize::MAX >> 1;
const FILLED: usize = usize::MAX ^ REFCOUNT_MASK;

impl<T> Default for Slot<T> {
    fn default() -> Self {
        Slot { state: AtomicUsize::new(0), val: UnsafeCell::new(None) }
    }
}

struct Inner<S: Stream> {
    buffer: Box<[Slot<S::Item>]>,
    stream: UnsafeCell<S>, // Should this be an Option, so we can drop it if we hit the end?
    poll_state: AtomicUsize,
    notifier: Arc<WakerSet>,
}

unsafe impl<S> Send for Inner<S>
where
    S: Stream + Send,
    S::Item: Send + Sync,
{
}

unsafe impl<S> Sync for Inner<S>
where
    S: Stream + Send,
    S::Item: Send + Sync,
{
}

const IDLE: usize = 0;
const POLLING: usize = 1;
const POISONED: usize = 2;

// The stream itself is polled behind the `Arc`, so it won't be moved
// when `Shared` is moved.
impl<S: Stream> Unpin for Shared<S> {}

impl<S: Stream> fmt::Debug for Inner<S> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("Inner").field("capacity", &(self.buffer.len() - 1)).finish()
    }
}

unsafe impl<T> Send for Slot<T> where T: Send {}
unsafe impl<T> Sync for Slot<T> where T: Send + Sync {}

/// If the idx is set to this, then the stream has reached the end.
const TERMINATED: usize = usize::MAX;

impl<S: Stream> Shared<S> {
    pub(super) fn new(stream: S, cap: usize) -> Shared<S> {
        assert!(cap > 0, "Shared stream must have capacity of at least 1");
        // Need an extra empty element for ring buffer
        let cap = cap + 1;
        assert!(cap < TERMINATED, "Capacity too large");
        let mut buffer = Vec::with_capacity(cap);
        // The first slot will initially have one reference to it.
        buffer.push(Slot {
            // Initial state is empty, with 1 reference
            state: AtomicUsize::new(1),
            val: UnsafeCell::new(None),
        });

        // The rest will initially be empty
        for _ in 0..cap {
            buffer.push(Slot::default());
        }

        Shared {
            idx: 0,
            waker_key: WakerKey::NULL,
            inner: Arc::new(Inner {
                buffer: buffer.into(),
                stream: UnsafeCell::new(stream),
                poll_state: AtomicUsize::new(IDLE),
                notifier: Arc::new(WakerSet::new()),
            }),
        }
    }
}

impl<S: Stream> Shared<S>
where
    S::Item: Clone,
{
    fn take_next(&mut self) -> Option<S::Item> {
        let (result, idx) = self.inner.take(self.idx);
        self.idx = idx;
        result
    }

    #[inline]
    fn slot(&self) -> &Slot<S::Item> {
        self.inner.slot(self.idx)
    }

    fn poll_stream(&mut self) -> Poll<Option<S::Item>> {
        if self.inner.fill_buffer(self.idx) {
            debug_assert!(self.slot().state.load(Relaxed) & FILLED != 0);
            Poll::Ready(self.take_next())
        } else {
            Poll::Pending
        }
    }
}

impl<S: Stream> Stream for Shared<S>
where
    S::Item: Clone,
{
    type Item = S::Item;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = &mut *self;
        if this.is_terminated() {
            return Poll::Ready(None);
        }
        if this.slot().is_full() {
            Poll::Ready(this.take_next())
        } else {
            this.inner.record_waker(&mut this.waker_key, cx);
            this.poll_stream()
        }
    }
}

impl<S: Stream> FusedStream for Shared<S>
where
    S::Item: Clone,
{
    fn is_terminated(&self) -> bool {
        self.idx == TERMINATED
    }
}

impl<S: Stream> Clone for Shared<S>
where
    S::Item: Clone,
{
    fn clone(&self) -> Self {
        let inner = self.inner.clone();
        assert!(
            Arc::strong_count(&inner) < REFCOUNT_MASK,
            "Created too many clones of shared stream"
        );
        if !self.is_terminated() {
            self.slot().inc_refcount();
        }
        Shared { idx: self.idx, waker_key: WakerKey::NULL, inner }
    }
}

impl<S: Stream> Drop for Shared<S> {
    fn drop(&mut self) {
        self.inner.notifier.unregister(self.waker_key);
        self.inner.drop_ref(self.idx);
    }
}

impl<S: Stream> Inner<S> {
    #[inline]
    fn cap(&self) -> usize {
        self.buffer.len()
    }

    fn prev_idx(&self, idx: usize) -> usize {
        match idx.checked_sub(1) {
            Some(prev) => prev,
            None => self.cap() - 1,
        }
    }
    fn next_idx(&self, idx: usize) -> usize {
        (idx + 1) % self.cap()
    }

    fn slot(&self, idx: usize) -> &Slot<S::Item> {
        &self.buffer[idx]
    }

    #[inline]
    fn is_first(&self, idx: usize) -> bool {
        let state = self.buffer[self.prev_idx(idx)].state.load(Acquire);
        state & FILLED == 0
    }

    /// Notify any waiting tasks that there is space available.
    fn notify_waiting(&self, idx: usize) {
        let slot = &self.buffer[self.prev_idx(idx)];
        let state = slot.state.load(Relaxed);
        if state & FILLED == 0 && state & REFCOUNT_MASK != 0 {
            ArcWake::wake_by_ref(&self.notifier);
        }
    }

    fn drop_ref(&self, idx: usize) {
        // drop_ref might be called when the idx is
        // TERMINATED, because it reached the end.
        if idx > self.buffer.len() {
            return;
        }
        let slot = &self.buffer[idx];
        let old = slot.state.fetch_sub(1, AcqRel);
        let refcount = old & REFCOUNT_MASK;
        debug_assert!(refcount > 0);

        // If we are the last reference to the first slot, then drop the slot.
        if refcount == 1 && self.is_first(idx) {
            // This is safe, because nothing else can access this slot if we are the last
            // thing to use it.
            unsafe { *slot.val.get() = None };
            // state is empty with no references
            slot.state.store(0, Release);
            self.notify_waiting(idx);
        }
    }

    fn record_waker(&self, waker_key: &mut WakerKey, cx: &mut Context<'_>) {
        self.notifier.record_waker(waker_key, cx);
    }

    /// Attempt to get a lock for polling
    ///
    /// Returns Ok with a lock guard if able to optain a lock
    /// and Err if the lock is already held.
    ///
    /// Panics if the state is poisoned
    fn try_lock(&self) -> Result<LockGuard<'_>, ()> {
        match self.poll_state.compare_exchange(IDLE, POLLING, Acquire, Relaxed) {
            Ok(_) => Ok(LockGuard { state: &self.poll_state, did_not_panic: true }),
            Err(POLLING) => Err(()),
            Err(POISONED) => panic!("inner stream panicked during poll"),
            _ => unreachable!(),
        }
    }

    /// Try polling the upstream stream to fill the buffer.
    ///
    /// This will keep polling until we either run out of space
    /// or the upstream stream is no longer ready. This reduces
    /// context switches.
    ///
    /// Returns true if at least one slot was filled (including by another task), false otherwise
    fn fill_buffer(&self, mut idx: usize) -> bool {
        let mut slot = self.slot(idx);
        let mut lock = match self.try_lock() {
            Ok(lock) => lock,
            // Even if we didn't get the lock, the slot might have been filled already
            Err(_) => return slot.is_full(),
        };
        // Check that the slot wasn't filled between the last check and possibly getting the lock
        if slot.is_full() {
            return true;
        }

        let waker = waker_ref(&self.notifier);
        let mut cx = Context::from_waker(&waker);

        let mut filled_one = false;

        loop {
            let next_idx = self.next_idx(idx);
            let next_slot = &self.buffer[next_idx];
            // Use relaxed ordering because, we don't actually read the value of the next slot
            if next_slot.state.load(Relaxed) & FILLED != 0 {
                // We have filled the buffer, return whether we
                // filled any other slots
                break;
            }
            let item = {
                // Safety:
                // Pin::new_unchecked is safe because the stream is inside an Arc, and is
                // never moved out of it.
                // dereferencing self.inner.stream.get() is safe because we currently hold a lock
                // from try_start_poll()
                let stream = unsafe { Pin::new_unchecked(&mut *self.stream.get()) };
                let res = lock.with_panic_check(|| stream.poll_next(&mut cx));
                match res {
                    Poll::Pending => {
                        return filled_one;
                    }
                    Poll::Ready(item) => item,
                }
            };
            let is_end = item.is_none();
            // Safety:
            // We have a lock that prevents any concurrent writes to any slots. And the full
            // state is currently false, so nothing else can be reading from this slot.
            unsafe { slot.fill(item) };
            filled_one = true;

            if is_end {
                drop(lock);
                self.notifier.wake_and_finish();
                return filled_one;
            }

            idx = next_idx;
            slot = next_slot;
        }
        if filled_one {
            self.notifier.wake_all();
            drop(lock);
        }
        filled_one
    }
}

impl<S: Stream> Inner<S>
where
    S::Item: Clone,
{
    fn take(&self, idx: usize) -> (Option<S::Item>, usize) {
        let slot = &self.buffer[idx];
        // if we've gotten this far, the slot is already filled.
        let state = slot.state.load(Acquire);
        debug_assert!(state & FILLED != 0, "Attempt to read from unfilled slot");

        // We need to increment the next slot first, to ensure that another thread
        // doesn't think it owns the next slot after we clear this slot.
        let next_idx = self.next_idx(idx);
        self.buffer[next_idx].inc_refcount();
        // This is valid because the previous buffer shouldn't be written to
        // until this one is empty.
        let value = if state == (FILLED | 1) && self.is_first(idx) {
            // This is safe because there is only a single
            // stream that still has access to this slot, and it
            // is the one currently taking a value out.
            let result = unsafe { (*slot.val.get()).take() };
            slot.state.store(0, Release);
            self.notify_waiting(idx);
            result
        } else {
            // This is safe because nothing else should write to this until
            // it is the first slot, and it is empty.
            let result = unsafe { (*slot.val.get()).clone() };
            self.drop_ref(idx);
            result
        };
        if value.is_some() {
            (value, next_idx)
        } else {
            // we've reached the end of the stream, so fuse the outer stream
            self.drop_ref(next_idx);
            (value, TERMINATED)
        }
    }
}

impl<T> Slot<T> {
    fn is_full(&self) -> bool {
        self.state.load(Acquire) & FILLED != 0
    }

    fn inc_refcount(&self) {
        let old = self.state.fetch_add(1, Release);
        assert!(old & REFCOUNT_MASK != REFCOUNT_MASK);
    }

    /// Store an item in the slot
    ///
    /// Safety:
    /// This function is only safe to call if it is safe to mutate the slot.
    /// That is if the caller has exclusive access to this slot.
    unsafe fn fill(&self, item: Option<T>) {
        *self.val.get() = item;
        let old = self.state.fetch_or(FILLED, Release);
        debug_assert!(old & FILLED == 0);
    }
}

struct LockGuard<'a> {
    state: &'a AtomicUsize,
    did_not_panic: bool,
}

impl LockGuard<'_> {
    fn with_panic_check<F, T>(&mut self, f: F) -> T
    where
        F: FnOnce() -> T,
    {
        self.did_not_panic = false;
        let res = f();
        self.did_not_panic = true;
        res
    }
}

impl Drop for LockGuard<'_> {
    fn drop(&mut self) {
        if self.did_not_panic {
            self.state.store(IDLE, Release)
        } else {
            self.state.store(POISONED, Release)
        }
    }
}
