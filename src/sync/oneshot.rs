//! A one-shot, futures-aware channel

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize};
use std::sync::atomic::Ordering::{AcqRel, Relaxed, Acquire, SeqCst};
use std::error::Error;
use std::fmt;
use std::cell::UnsafeCell;

use {Future, Poll, Async};
use future::{lazy, Lazy, Executor, IntoFuture};
use task::{self, Task};

/// A future representing the completion of a computation happening elsewhere in
/// memory.
///
/// This is created by the `oneshot::channel` function.
#[must_use = "futures do nothing unless polled"]
#[derive(Debug)]
pub struct Receiver<T> {
    inner: Arc<Inner<T>>,
}

/// Represents the completion half of a oneshot through which the result of a
/// computation is signaled.
///
/// This is created by the `oneshot::channel` function.
#[derive(Debug)]
pub struct Sender<T> {
    inner: Arc<Inner<T>>,
}

/// Internal state of the `Receiver`/`Sender` pair above. This is all used as
/// the internal synchronization between the two for send/recv operations.
#[derive(Debug)]
struct Inner<T> {
    state: AtomicUsize,
    // Four task handles are needed (two for each side)
    tx_tasks: [UnsafeCell<Option<Task>>; 2],
    rx_tasks: [UnsafeCell<Option<Task>>; 2],
    // Access to the value field is allowed according to this table:
    // +----------+------------+--------+----------+------------+
    // | SENT_BIT | CLOSED_BIT |  SENDER | RECEIVER | STATE NAME |
    // +----------+------------+---------+----------+------------+
    // |     0    |      0     |   Yes   |    No    |    Open    |
    // |     0    |      1     |   Yes   |    No    |   Closed   |
    // |     1    |      0     |   No    |    Yes   |    Sent    |
    // |     1    |      1     | Closed? |   Sent?  |  Complete  |
    // +----------+------------+---------+----------+------------+
    // This is the state diagram:
    //
    // (Open) -> Sent -> Complete
    //     \             ^
    //      `-> Closed -'
    //
    // Three operations are relevant:
    // - `send()` [Sender]
    //   Can occur whilst in the "Open" or "Closed" states, so sender has access to
    //   value before state transition. After state transition, value is only
    //   accessed if the previous state was "Closed".
    //
    // - `close()` [Receiver]
    //   Transitions to either "Closed" or "Complete" states.
    //   If previous state was "Sent", sets the "ORPHAN_BIT", indicating a value
    //   has been orphaned in the data structure.
    //
    // - `recv()` [Receiver]
    //   Only accesses the value if it observes the "Sent" state, or if the "ORPHAN_BIT"
    //   is set. Since only the receiver can transition away from the "Sent" state, this
    //   cannot race with other operations. If the "ORPHAN_BIT" is set, the sender has
    //   already gone away.
    value: UnsafeCell<Option<T>>,
}

unsafe impl<T: Send> Send for Inner<T> {}
unsafe impl<T: Send> Sync for Inner<T> {}

/// Creates a new futures-aware, one-shot channel.
///
/// This function is similar to Rust's channels found in the standard library.
/// Two halves are returned, the first of which is a `Sender` handle, used to
/// signal the end of a computation and provide its value. The second half is a
/// `Receiver` which implements the `Future` trait, resolving to the value that
/// was given to the `Sender` handle.
///
/// Each half can be separately owned and sent across threads/tasks.
///
/// # Examples
///
/// ```
/// use std::thread;
/// use futures::sync::oneshot;
/// use futures::*;
///
/// let (p, c) = oneshot::channel::<i32>();
///
/// thread::spawn(|| {
///     c.map(|i| {
///         println!("got: {}", i);
///     }).wait();
/// });
///
/// p.send(3).unwrap();
/// ```
pub fn channel<T>() -> (Sender<T>, Receiver<T>) {
    let inner = Arc::new(Inner::new());
    let receiver = Receiver {
        inner: inner.clone(),
    };
    let sender = Sender {
        inner: inner,
    };
    (sender, receiver)
}

unsafe fn notify_task(loc: &UnsafeCell<Option<Task>>) {
    if let Some(task) = (*loc.get()).take() {
        task.notify();
    }
}

unsafe fn store_task(loc: &UnsafeCell<Option<Task>>) {
    *loc.get() = Some(task::current());
}

fn get_bit(state: usize, bit: usize) -> usize {
    (state >> bit) & 1
}

const RX_BIT: usize = 0;
const TX_BIT: usize = 1;
const SENT_BIT: usize = 2;
const CLOSED_BIT: usize = 3;
const ORPHAN_BIT: usize = 4;

impl<T> Inner<T> {
    fn new() -> Inner<T> {
        Inner {
            state: Default::default(),
            tx_tasks: Default::default(),
            rx_tasks: Default::default(),
            value: Default::default(),
        }
    }

    unsafe fn send(&self, t: T) -> Result<(), T> {
        *self.value.get() = Some(t);
        let prev_state = self.state.fetch_or(1 << SENT_BIT, AcqRel);
        if get_bit(prev_state, CLOSED_BIT) == 1 {
            // Receiver has already closed the channel
            Err((*self.value.get()).take().unwrap())
        } else {
            // Wake up receiver
            notify_task(&self.rx_tasks[get_bit(prev_state, RX_BIT)^1]);
            Ok(())
        }
    }

    unsafe fn poll_cancel(&self) -> Poll<(), ()> {
        // Fast path
        let mut prev_state = self.state.load(Acquire);
        if get_bit(prev_state, CLOSED_BIT) == 1 {
            Ok(Async::Ready(()))
        } else {
            // Save task handle
            store_task(&self.tx_tasks[get_bit(prev_state, TX_BIT)]);

            // Check again
            prev_state = self.state.fetch_xor(1 << TX_BIT, AcqRel);
            if get_bit(prev_state, CLOSED_BIT) == 1 {
                Ok(Async::Ready(()))
            } else {
                Ok(Async::NotReady)
            }
        }
    }

    fn is_canceled(&self) -> bool {
        get_bit(self.state.load(Acquire), CLOSED_BIT) == 1
    }

    unsafe fn close_tx(&self) {
        let prev_state = self.state.fetch_or(1 << SENT_BIT, AcqRel);
        if get_bit(prev_state, SENT_BIT) | get_bit(prev_state, CLOSED_BIT) == 0 {
            // Wake up receiver
            notify_task(&self.rx_tasks[get_bit(prev_state, RX_BIT)^1]);
        }
    }

    unsafe fn close_rx(&self) {
        let prev_state = self.state.fetch_or(1 << CLOSED_BIT, AcqRel);
        if get_bit(prev_state, CLOSED_BIT) == 0 {
            if get_bit(prev_state, SENT_BIT) == 1 {
                // Mark the data structure as containing an orphaned value
                self.state.fetch_or(1 << ORPHAN_BIT, Relaxed);
            } else {
                // Wake up sender
                notify_task(&self.tx_tasks[get_bit(prev_state, TX_BIT)^1]);
            }
        }
    }

    unsafe fn recv(&self) -> Poll<T, Canceled> {
        // Fast path
        let mut prev_state = self.state.load(Acquire);
        if get_bit(prev_state, CLOSED_BIT) == 1 {
            if get_bit(prev_state, ORPHAN_BIT) == 1 {
                (*self.value.get()).take().ok_or(Canceled).map(Async::Ready)
            } else {
                Err(Canceled)
            }
        } else if get_bit(prev_state, SENT_BIT) == 1 {
            (*self.value.get()).take().ok_or(Canceled).map(Async::Ready)
        } else {
            // Save task handle
            store_task(&self.rx_tasks[get_bit(prev_state, RX_BIT)]);

            // Check again
            prev_state = self.state.fetch_xor(1 << RX_BIT, AcqRel);
            if get_bit(prev_state, SENT_BIT) == 1 {
                (*self.value.get()).take().ok_or(Canceled).map(Async::Ready)
            } else {
                Ok(Async::NotReady)
            }
        }
    }
}

impl<T> Sender<T> {
    #[deprecated(note = "renamed to `send`", since = "0.1.11")]
    #[doc(hidden)]
    #[cfg(feature = "with-deprecated")]
    pub fn complete(self, t: T) {
        drop(self.send(t));
    }

    /// Completes this oneshot with a successful result.
    ///
    /// This function will consume `self` and indicate to the other end, the
    /// `Receiver`, that the value provided is the result of the computation this
    /// represents.
    ///
    /// If the value is successfully enqueued for the remote end to receive,
    /// then `Ok(())` is returned. If the receiving end was deallocated before
    /// this function was called, however, then `Err` is returned with the value
    /// provided.
    pub fn send(self, t: T) -> Result<(), T> {
        unsafe { self.inner.send(t) }
    }

    /// Polls this `Sender` half to detect whether the `Receiver` this has
    /// paired with has gone away.
    ///
    /// This function can be used to learn about when the `Receiver` (consumer)
    /// half has gone away and nothing will be able to receive a message sent
    /// from `send`.
    ///
    /// If `Ready` is returned then it means that the `Receiver` has disappeared
    /// and the result this `Sender` would otherwise produce should no longer
    /// be produced.
    ///
    /// If `NotReady` is returned then the `Receiver` is still alive and may be
    /// able to receive a message if sent. The current task, however, is
    /// scheduled to receive a notification if the corresponding `Receiver` goes
    /// away.
    ///
    /// # Panics
    ///
    /// Like `Future::poll`, this function will panic if it's not called from
    /// within the context of a task. In other words, this should only ever be
    /// called from inside another future.
    ///
    /// If you're calling this function from a context that does not have a
    /// task, then you can use the `is_canceled` API instead.
    pub fn poll_cancel(&mut self) -> Poll<(), ()> {
        unsafe { self.inner.poll_cancel() }
    }

    /// Tests to see whether this `Sender`'s corresponding `Receiver`
    /// has gone away.
    ///
    /// This function can be used to learn about when the `Receiver` (consumer)
    /// half has gone away and nothing will be able to receive a message sent
    /// from `send`.
    ///
    /// Note that this function is intended to *not* be used in the context of a
    /// future. If you're implementing a future you probably want to call the
    /// `poll_cancel` function which will block the current task if the
    /// cancellation hasn't happened yet. This can be useful when working on a
    /// non-futures related thread, though, which would otherwise panic if
    /// `poll_cancel` were called.
    pub fn is_canceled(&self) -> bool {
        self.inner.is_canceled()
    }
}

impl<T> Drop for Sender<T> {
    fn drop(&mut self) {
        unsafe { self.inner.close_tx() }
    }
}

/// Error returned from a `Receiver<T>` whenever the corresponding `Sender<T>`
/// is dropped.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Canceled;

impl fmt::Display for Canceled {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "oneshot canceled")
    }
}

impl Error for Canceled {
    fn description(&self) -> &str {
        "oneshot canceled"
    }
}

impl<T> Receiver<T> {
    /// Gracefully close this receiver, preventing sending any future messages.
    ///
    /// Any `send` operation which happens after this method returns is
    /// guaranteed to fail. Once this method is called the normal `poll` method
    /// can be used to determine whether a message was actually sent or not. If
    /// `Canceled` is returned from `poll` then no message was sent.
    pub fn close(&mut self) {
        unsafe { self.inner.close_rx() }
    }
}

impl<T> Future for Receiver<T> {
    type Item = T;
    type Error = Canceled;

    fn poll(&mut self) -> Poll<T, Canceled> {
        unsafe { self.inner.recv() }
    }
}

impl<T> Drop for Receiver<T> {
    fn drop(&mut self) {
        unsafe { self.inner.close_rx() }
    }
}

/// Handle returned from the `spawn` function.
///
/// This handle is a future representing the completion of a different future on
/// a separate executor. Created through the `oneshot::spawn` function this
/// handle will resolve when the future provided to `spawn` resolves on the
/// `Executor` instance provided to that function.
///
/// If this handle is dropped then the future will automatically no longer be
/// polled and is scheduled to be dropped. This can be canceled with the
/// `forget` function, however.
pub struct SpawnHandle<T, E> {
    rx: Arc<ExecuteInner<Result<T, E>>>,
}

struct ExecuteInner<T> {
    inner: Inner<T>,
    keep_running: AtomicBool,
}

/// Type of future which `Execute` instances below must be able to spawn.
pub struct Execute<F: Future> {
    future: F,
    tx: Arc<ExecuteInner<Result<F::Item, F::Error>>>,
}

/// Spawns a `future` onto the instance of `Executor` provided, `executor`,
/// returning a handle representing the completion of the future.
///
/// The `SpawnHandle` returned is a future that is a proxy for `future` itself.
/// When `future` completes on `executor` then the `SpawnHandle` will itself be
/// resolved.  Internally `SpawnHandle` contains a `oneshot` channel and is
/// thus safe to send across threads.
///
/// The `future` will be canceled if the `SpawnHandle` is dropped. If this is
/// not desired then the `SpawnHandle::forget` function can be used to continue
/// running the future to completion.
///
/// # Panics
///
/// This function will panic if the instance of `Spawn` provided is unable to
/// spawn the `future` provided.
///
/// If the provided instance of `Spawn` does not actually run `future` to
/// completion, then the returned handle may panic when polled. Typically this
/// is not a problem, though, as most instances of `Spawn` will run futures to
/// completion.
///
/// Note that the returned future will likely panic if the `futures` provided
/// panics. If a future running on an executor panics that typically means that
/// the executor drops the future, which falls into the above case of not
/// running the future to completion essentially.
pub fn spawn<F, E>(future: F, executor: &E) -> SpawnHandle<F::Item, F::Error>
    where F: Future,
          E: Executor<Execute<F>>,
{
    let data = Arc::new(ExecuteInner {
        inner: Inner::new(),
        keep_running: AtomicBool::new(false),
    });
    executor.execute(Execute {
        future: future,
        tx: data.clone(),
    }).expect("failed to spawn future");
    SpawnHandle { rx: data }
}

/// Spawns a function `f` onto the `Spawn` instance provided `s`.
///
/// For more information see the `spawn` function in this module. This function
/// is just a thin wrapper around `spawn` which will execute the closure on the
/// executor provided and then complete the future that the closure returns.
pub fn spawn_fn<F, R, E>(f: F, executor: &E) -> SpawnHandle<R::Item, R::Error>
    where F: FnOnce() -> R,
          R: IntoFuture,
          E: Executor<Execute<Lazy<F, R>>>,
{
    spawn(lazy(f), executor)
}

impl<T, E> SpawnHandle<T, E> {
    /// Drop this future without canceling the underlying future.
    ///
    /// When `SpawnHandle` is dropped, the spawned future will be canceled as
    /// well if the future hasn't already resolved. This function can be used
    /// when to drop this future but keep executing the underlying future.
    pub fn forget(self) {
        self.rx.keep_running.store(true, SeqCst);
    }
}

impl<T, E> Future for SpawnHandle<T, E> {
    type Item = T;
    type Error = E;

    fn poll(&mut self) -> Poll<T, E> {
        match unsafe { self.rx.inner.recv() } {
            Ok(Async::Ready(Ok(t))) => Ok(t.into()),
            Ok(Async::Ready(Err(e))) => Err(e),
            Ok(Async::NotReady) => Ok(Async::NotReady),
            Err(_) => panic!("future was canceled before completion"),
        }
    }
}

impl<T: fmt::Debug, E: fmt::Debug> fmt::Debug for SpawnHandle<T, E> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("SpawnHandle")
         .finish()
    }
}

impl<T, E> Drop for SpawnHandle<T, E> {
    fn drop(&mut self) {
        unsafe { self.rx.inner.close_rx(); }
    }
}

impl<F: Future> Future for Execute<F> {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Poll<(), ()> {
        // If we're canceled then we may want to bail out early.
        //
        // If the `forget` function was called, though, then we keep going.
        if unsafe { self.tx.inner.poll_cancel() }.unwrap().is_ready() {
            if !self.tx.keep_running.load(SeqCst) {
                return Ok(().into())
            }
        }

        let result = match self.future.poll() {
            Ok(Async::NotReady) => return Ok(Async::NotReady),
            Ok(Async::Ready(t)) => Ok(t),
            Err(e) => Err(e),
        };
        drop(unsafe { self.tx.inner.send(result) });
        Ok(().into())
    }
}

impl<F: Future + fmt::Debug> fmt::Debug for Execute<F> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Execute")
         .field("future", &self.future)
         .finish()
    }
}

impl<F: Future> Drop for Execute<F> {
    fn drop(&mut self) {
        unsafe { self.tx.inner.close_tx() };
    }
}
