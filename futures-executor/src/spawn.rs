use futures_core::{Future, Async, Poll};
use futures_core::never::Never;
use futures_core::task::{self, Context};
use futures_channel::oneshot::{channel, Sender, Receiver};
use futures_util::FutureExt;

use std::thread;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::panic::{self, AssertUnwindSafe};
use std::sync::atomic::AtomicBool;

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
    where F: Future<Item = (), Error = Never> + 'static + Send
{
    Spawn(Some(f))
}

impl<F: Future<Item = (), Error = Never> + Send + 'static> Future for Spawn<F> {
    type Item = ();
    type Error = Never;
    fn poll(&mut self, cx: &mut Context) -> Poll<(), Never> {
        cx.spawn(self.0.take().unwrap());
        Ok(Async::Ready(()))
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
pub fn spawn_with_handle<F>(f: F) -> SpawnWithHandle<F>
    where F: Future + 'static + Send, F::Item: Send, F::Error: Send
{
    SpawnWithHandle(Some(f))
}

impl<F> Future for SpawnWithHandle<F>
    where F: Future<Item = (), Error = Never> + Send + 'static,
          F::Item: Send,
          F::Error: Send,
{
    type Item = JoinHandle<F::Item, F::Error>;
    type Error = Never;
    fn poll(&mut self, cx: &mut Context) -> Poll<Self::Item, Never> {
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
        Ok(Async::Ready(JoinHandle {
            inner: rx ,
            keep_running_flag: keep_running_flag.clone()
        }))
    }
}

struct MySender<F, T> {
    fut: F,
    tx: Option<Sender<T>>,
    keep_running_flag: Arc<AtomicBool>,
}

/// The type of future returned from the `ThreadPool::spawn` function, which
/// proxies the futures running on the thread pool.
///
/// This future will resolve in the same way as the underlying future, and it
/// will propagate panics.
#[must_use]
#[derive(Debug)]
pub struct JoinHandle<T, E> {
    inner: Receiver<thread::Result<Result<T, E>>>,
    keep_running_flag: Arc<AtomicBool>,
}

impl<T, E> JoinHandle<T, E> {
    /// Drop this handle *without* canceling the underlying future.
    ///
    /// When `JoinHandle` is dropped, `ThreadPool` will try to abort the associated
    /// task. This function can be used when you want to drop the handle but keep
    /// executing the task.
    pub fn forget(self) {
        self.keep_running_flag.store(true, Ordering::SeqCst);
    }
}

impl<T: Send + 'static, E: Send + 'static> Future for JoinHandle<T, E> {
    type Item = T;
    type Error = E;

    fn poll(&mut self, cx: &mut task::Context) -> Poll<T, E> {
        match self.inner.poll(cx).expect("cannot poll JoinHandle twice") {
            Async::Ready(Ok(Ok(e))) => Ok(e.into()),
            Async::Ready(Ok(Err(e))) => Err(e),
            Async::Ready(Err(e)) => panic::resume_unwind(e),
            Async::Pending => Ok(Async::Pending),
        }
    }
}

impl<F: Future> Future for MySender<F, Result<F::Item, F::Error>> {
    type Item = ();
    type Error = Never;

    fn poll(&mut self, cx: &mut task::Context) -> Poll<(), Never> {
        if let Ok(Async::Ready(_)) = self.tx.as_mut().unwrap().poll_cancel(cx) {
            if !self.keep_running_flag.load(Ordering::SeqCst) {
                // Cancelled, bail out
                return Ok(().into())
            }
        }

        let res = match self.fut.poll(cx) {
            Ok(Async::Ready(e)) => Ok(e),
            Ok(Async::Pending) => return Ok(Async::Pending),
            Err(e) => Err(e),
        };

        // if the receiving end has gone away then that's ok, we just ignore the
        // send error here.
        drop(self.tx.take().unwrap().send(res));
        Ok(Async::Ready(()))
    }
}
