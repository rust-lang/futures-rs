use crate::future::FutureExt;
use super::SpawnError;
use futures_channel::oneshot::{self, Sender, Receiver};
use futures_core::future::Future;
use futures_core::task::{self, Poll, Executor, SpawnObjError};
use pin_utils::{unsafe_pinned, unsafe_unpinned};
use std::marker::Unpin;
use std::mem::PinMut;
use std::panic::{self, AssertUnwindSafe};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

/// The join handle returned by
/// [`spawn_with_handle`](crate::task::ExecutorExt::spawn_with_handle).
#[must_use = "futures do nothing unless polled"]
#[derive(Debug)]
pub struct JoinHandle<T> {
    rx: Receiver<thread::Result<T>>,
    keep_running: Arc<AtomicBool>,
}

impl<T> JoinHandle<T> {
    /// Drops this handle *without* canceling the underlying future.
    ///
    /// This method can be used if you want to drop the handle, but let the
    /// execution continue.
    pub fn forget(self) {
        self.keep_running.store(true, Ordering::SeqCst);
    }
}

impl<T: Send + 'static> Future for JoinHandle<T> {
    type Output = T;

    fn poll(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<T> {
        match self.rx.poll_unpin(cx) {
            Poll::Ready(Ok(Ok(output))) => Poll::Ready(output),
            Poll::Ready(Ok(Err(e))) => panic::resume_unwind(e),
            Poll::Ready(Err(e)) => panic::resume_unwind(Box::new(e)),
            Poll::Pending => Poll::Pending,
        }
    }
}

struct Wrapped<Fut: Future> {
    tx: Option<Sender<Fut::Output>>,
    keep_running: Arc<AtomicBool>,
    future: Fut,
}

impl<Fut: Future + Unpin> Unpin for Wrapped<Fut> {}

impl<Fut: Future> Wrapped<Fut> {
    unsafe_pinned!(future: Fut);
    unsafe_unpinned!(tx: Option<Sender<Fut::Output>>);
    unsafe_unpinned!(keep_running: Arc<AtomicBool>);
}

impl<Fut: Future> Future for Wrapped<Fut> {
    type Output = ();

    fn poll(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<()> {
        if let Poll::Ready(_) = self.tx().as_mut().unwrap().poll_cancel(cx) {
            if !self.keep_running().load(Ordering::SeqCst) {
                // Cancelled, bail out
                return Poll::Ready(())
            }
        }

        let output = match self.future().poll(cx) {
            Poll::Ready(output) => output,
            Poll::Pending => return Poll::Pending,
        };

        // if the receiving end has gone away then that's ok, we just ignore the
        // send error here.
        drop(self.tx().take().unwrap().send(output));
        Poll::Ready(())
    }
}

pub(super) fn spawn_with_handle<Ex, Fut>(
    executor: &mut Ex,
    future: Fut,
) -> Result<JoinHandle<Fut::Output>, SpawnError>
where Ex: Executor + ?Sized,
      Fut: Future + Send + 'static,
      Fut::Output: Send,
{
    let (tx, rx) = oneshot::channel();
    let keep_running = Arc::new(AtomicBool::new(false));

    // AssertUnwindSafe is used here because `Send + 'static` is basically
    // an alias for an implementation of the `UnwindSafe` trait but we can't
    // express that in the standard library right now.
    let wrapped = Wrapped {
        future: AssertUnwindSafe(future).catch_unwind(),
        tx: Some(tx),
        keep_running: keep_running.clone(),
    };

    let res = executor.spawn_obj(Box::new(wrapped).into());
    match res {
        Ok(()) => Ok(JoinHandle { rx, keep_running }),
        Err(SpawnObjError { kind, .. }) => Err(SpawnError { kind }),
    }
}
