use {
    crate::future::{CatchUnwind, FutureExt},
    futures_channel::oneshot::{self, Sender, Receiver},
    futures_core::{
        future::Future,
        task::{Context, Poll},
    },
    pin_utils::{unsafe_pinned, unsafe_unpinned},
    std::{
        any::Any,
        fmt,
        panic::{AssertUnwindSafe},
        pin::Pin,
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
        thread,
    },
};

/// The handle to a remote future returned by
/// [`remote_handle`](crate::future::FutureExt::remote_handle).
///
/// The RemoteHandle resolves to an [std::thread::Result] over the output
/// of the future.
///
/// ## Drop behavior
/// When you drop RemoteHandle, the remote future will be woken up to
/// be dropped by the executor. Else, if the remote executor drops the future
/// before polling it to completion, the [std::thread::Result] RemoteHandle
/// resolves to will contain a [futures_channel::oneshot::Canceled].
///
/// You can detach the handle from the future by calling [RemoteHandle::forget]. This
/// will let the future continue to be polled by the executor, even when
/// you drop the RemoteHandle.
///
/// ## Panics
/// If the remote future panics, the error passed to the panic is sent to the
/// RemoteHandle. You can downcast the error to check what went wrong.
///
/// The thread on which the panic happened will unwind.
///
/// ### Example
/// ```
/// use futures::executor::{block_on, ThreadPool};
/// use futures::FutureExt;
/// use futures::task::SpawnExt;
/// use std::fs::File;
///
/// let executor = ThreadPool::new().unwrap();
///
/// // Only by explicitly passing the error to the panic! macro can we downcast it later.
/// let future = async { panic!(File::open("doesnotexist").unwrap_err()); };
///
/// // for reference, this would print:
/// // "got String: called `Result::unwrap()` on an `Err` value: Os { code: 2,
/// // kind: NotFound, message: "No such file or directory" }"
/// //
/// // let future = async { File::open("doesnotexist").unwrap(); };
///
/// // This will print:
/// // "got String: expect string: Os { code: 2, kind: NotFound,
/// // message: "No such file or directory" }"
/// //
/// // let future = async { File::open("doesnotexist").expect("expect string"); };
///
/// let (task, handle) = future.remote_handle();
/// executor.spawn(task).unwrap();
/// let output = block_on(handle);
///
/// match output
/// {
///   Ok(_file) => unreachable!(), // our file doesn't exist, so this can't happen.
///
///   Err(e) => {
///       if let Some(err) = e.downcast_ref::<std::io::Error>() {
///           assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
///       }
///       else if let Some(err) = e.downcast_ref::<String>() {
///         println!("got String: {}", err);
///         unreachable!();
///       }
///       else if let Some(_err) = e.downcast_ref::<futures_channel::oneshot::Canceled>() {
///           println!("Executor dropped remote future before completion.");
///           unreachable!();
///       }
///       else { unreachable!(); }
///   }
/// }
/// ```
///
#[must_use = "futures do nothing unless you `.await` or poll them"]
#[derive(Debug)]
pub struct RemoteHandle<T> {
    rx: Receiver<thread::Result<T>>,
    keep_running: Arc<AtomicBool>,
}

impl<T> RemoteHandle<T> {
    /// Drops this handle *without* canceling the underlying future.
    ///
    /// This method can be used if you want to drop the handle, but let the
    /// execution continue.
    pub fn forget(self) {
        self.keep_running.store(true, Ordering::SeqCst);
    }
}

impl<T: Send + 'static> Future for RemoteHandle<T> {
    type Output = std::thread::Result<T>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match ready!(self.rx.poll_unpin(cx)) {
            Ok(Ok(output)) => Poll::Ready(Ok(output)),
            // this is when the future panicked.
            Ok(Err(e)) => Poll::Ready(Err(e)),
            // this is when the channel sender has been dropped. This can
            // happen if the executor drops the future before driving it to completion.
            Err(e) => Poll::Ready(Err(Box::new(e))),
        }
    }
}

type SendMsg<Fut> = Result<<Fut as Future>::Output, Box<(dyn Any + Send + 'static)>>;

/// A future which sends its output to the corresponding `RemoteHandle`.
/// Created by [`remote_handle`](crate::future::FutureExt::remote_handle).
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct Remote<Fut: Future> {
    tx: Option<Sender<SendMsg<Fut>>>,
    keep_running: Arc<AtomicBool>,
    future: CatchUnwind<AssertUnwindSafe<Fut>>,
}

impl<Fut: Future + fmt::Debug> fmt::Debug for Remote<Fut> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Remote")
            .field(&self.future)
            .finish()
    }
}

impl<Fut: Future + Unpin> Unpin for Remote<Fut> {}

impl<Fut: Future> Remote<Fut> {
    unsafe_pinned!(future: CatchUnwind<AssertUnwindSafe<Fut>>);
    unsafe_unpinned!(tx: Option<Sender<SendMsg<Fut>>>);
}

impl<Fut: Future> Future for Remote<Fut> {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        if let Poll::Ready(_) = self.as_mut().tx().as_mut().unwrap().poll_canceled(cx) {
            if !self.keep_running.load(Ordering::SeqCst) {
                // Cancelled, bail out
                return Poll::Ready(())
            }
        }

        // Did the future panic?...
        match ready!(self.as_mut().future().poll(cx))
        {
            // ...no
            Ok(value) => {
                // if the receiving end has gone away then that's ok, we just ignore the
                // send error here.
                drop(self.as_mut().tx().take().unwrap().send(Ok(value)));
                Poll::Ready(())
            }
            // ...yes, Send the error to the RemoteHandle and then tear down this thread.
            Err(e) => {
                let msg = format!("Remote future panicked with: {:?}", &e);
                drop(self.as_mut().tx().take().unwrap().send(Err(e)));
                std::panic::resume_unwind(Box::new(msg));
            }
        }
    }
}

pub(super) fn remote_handle<Fut: Future>(future: Fut) -> (Remote<Fut>, RemoteHandle<Fut::Output>) {
    let (tx, rx) = oneshot::channel();
    let keep_running = Arc::new(AtomicBool::new(false));

    // Safety:
    // We catch_unwind the future so that we can propagate the error from the
    // future to the thread awaiting the handle. However we don't want to make any
    // claims about it being safe to keep running, so we tear down the thread
    // that panicked with resume_unwind after passing on the error.
    let wrapped = Remote {
        future: AssertUnwindSafe(future).catch_unwind(),
        tx: Some(tx),
        keep_running: keep_running.clone(),
    };

    (wrapped, RemoteHandle { rx, keep_running })
}
