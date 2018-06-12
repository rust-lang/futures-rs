use futures_core::{Future, Poll};
use futures_core::task::Context;
use futures_channel::oneshot::{channel, Sender, Receiver};
use futures_util::FutureExt;

use std::thread;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::panic::{self, AssertUnwindSafe};
use std::sync::atomic::AtomicBool;
use std::mem::PinMut;
use std::boxed::Box;
use std::marker::Unpin;

/// A future representing the completion of task spawning.
///
/// See [`spawn`](spawn()) for details.
#[derive(Debug)]
pub struct Spawn<F>(Option<F>);

/// Spawn a task onto the default executor.
///
/// This function returns a future that will spawn the given future as a task
/// onto the default executor. It does *not* provide any way to wait on task
/// completion or extract a value from the task. That can either be done through
/// a channel, or by using [`spawn_with_handle`](::spawn_with_handle).
pub fn spawn<F>(f: F) -> Spawn<F>
    where F: Future<Output = ()> + 'static + Send
{
    Spawn(Some(f))
}

impl<F: Future<Output = ()> + Unpin + Send + 'static> Future for Spawn<F> {
    type Output = ();

    fn poll(mut self: PinMut<Self>, cx: &mut Context) -> Poll<()> {
        cx.spawn(self.0.take().unwrap());
        Poll::Ready(())
    }
}

/// A future representing the completion of task spawning, yielding a
/// [`JoinHandle`](::JoinHandle) to the spawned task.
///
/// See [`spawn_with_handle`](::spawn_with_handle) for details.
#[derive(Debug)]
pub struct SpawnWithHandle<F>(Option<F>);

/// Spawn a task onto the default executor, yielding a
/// [`JoinHandle`](::JoinHandle) to the spawned task.
///
/// This function returns a future that will spawn the given future as a task
/// onto the default executor. On completion, that future will yield a
/// [`JoinHandle`](::JoinHandle) that can itself be used as a future
/// representing the completion of the spawned task.
///
/// # Examples
///
/// ```
/// # extern crate futures;
/// #
/// use futures::prelude::*;
/// use futures::future;
/// use futures::executor::{block_on, spawn_with_handle};
///
/// # fn main() {
/// # fn inner() -> Result<(), Never> {
/// # Ok({
/// let future = future::ok::<u32, Never>(1);
/// let join_handle = block_on(spawn_with_handle(future))?;
/// let result = block_on(join_handle);
/// assert_eq!(result, Ok(1));
/// # })
/// # }
/// # inner().unwrap();
/// # }
/// ```
///
/// ```
/// # extern crate futures;
/// #
/// use futures::prelude::*;
/// use futures::future;
/// use futures::executor::{block_on, spawn_with_handle};
///
/// # fn main() {
/// # fn inner() -> Result<(), Never> {
/// # Ok({
/// let future = future::err::<Never, &str>("boom");
/// let join_handle = block_on(spawn_with_handle(future))?;
/// let result = block_on(join_handle);
/// assert_eq!(result, Err("boom"));
/// # })
/// # }
/// # inner().unwrap();
/// # }
/// ```
pub fn spawn_with_handle<F>(f: F) -> SpawnWithHandle<F>
    where F: Future + 'static + Send, F::Output: Send
{
    SpawnWithHandle(Some(f))
}

impl<F> Future for SpawnWithHandle<F>
    where F: Future + Unpin + Send + 'static,
          F::Output: Send,
{
    type Output = JoinHandle<F::Output>;

    fn poll(mut self: PinMut<Self>, cx: &mut Context) -> Poll<Self::Output> {
        let (tx, rx) = channel();
        let keep_running_flag = Arc::new(AtomicBool::new(false));
        // AssertUnwindSafe is used here because `Send + 'static` is basically
        // an alias for an implementation of the `UnwindSafe` trait but we can't
        // express that in the standard library right now.
        let sender = MySender {
            fut: AssertUnwindSafe(self.0.take().unwrap()).catch_unwind(),
            tx: Some(tx),
            keep_running_flag: keep_running_flag.clone(),
        };

        cx.spawn(sender);
        Poll::Ready(JoinHandle {
            inner: rx ,
            keep_running_flag: keep_running_flag.clone()
        })
    }
}

struct MySender<F, T> {
    fut: F,
    tx: Option<Sender<T>>,
    keep_running_flag: Arc<AtomicBool>,
}
impl<F, T> Unpin for MySender<F, T> {} // ToDo: May I do this?

/// The type of future returned from the `ThreadPool::spawn` function, which
/// proxies the futures running on the thread pool.
///
/// This future will resolve in the same way as the underlying future, and it
/// will propagate panics.
#[must_use]
#[derive(Debug)]
pub struct JoinHandle<T> {
    inner: Receiver<thread::Result<T>>,
    keep_running_flag: Arc<AtomicBool>,
}

impl<T> JoinHandle<T> {
    /// Drop this handle *without* canceling the underlying future.
    ///
    /// When `JoinHandle` is dropped, `ThreadPool` will try to abort the associated
    /// task. This function can be used when you want to drop the handle but keep
    /// executing the task.
    pub fn forget(self) {
        self.keep_running_flag.store(true, Ordering::SeqCst);
    }
}

impl<T: Send + 'static> Future for JoinHandle<T> {
    type Output = T;

    fn poll(mut self: PinMut<Self>, cx: &mut Context) -> Poll<T> { // ToDo: This was weird! Double check!
        match self.inner.poll_unpin(cx) {
            Poll::Ready(Ok(Ok(output))) => Poll::Ready(output),
            Poll::Ready(Ok(Err(e))) => panic::resume_unwind(e),
            Poll::Ready(Err(e)) => panic::resume_unwind(Box::new(e)),
            Poll::Pending => Poll::Pending,
        }
    }
}

impl<F: Future + Unpin> Future for MySender<F, F::Output> {
    type Output = ();

    fn poll(mut self: PinMut<Self>, cx: &mut Context) -> Poll<()> {
        if let Poll::Ready(_) = self.tx.as_mut().unwrap().poll_cancel(cx) {
            if !self.keep_running_flag.load(Ordering::SeqCst) {
                // Cancelled, bail out
                return Poll::Ready(())
            }
        }

        let output = match self.fut.poll_unpin(cx) {
            Poll::Ready(output) => output,
            Poll::Pending => return Poll::Pending,
        };

        // if the receiving end has gone away then that's ok, we just ignore the
        // send error here.
        drop(self.tx.take().unwrap().send(output));
        Poll::Ready(())
    }
}
