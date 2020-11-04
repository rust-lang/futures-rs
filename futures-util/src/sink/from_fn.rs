use core::{future::Future, pin::Pin};
use futures_core::task::{Context, Poll};
use futures_sink::Sink;
use pin_project::pin_project;

/// Sink for the [`from_fn`] function.
#[pin_project]
#[derive(Debug)]
#[must_use = "sinks do nothing unless polled"]
pub struct FromFn<F, R> {
    function: F,
    #[pin]
    future: Option<R>,
}

/// Create a sink from a function which processes one item at a time.
///
/// # Examples
///
/// ```
/// # futures::executor::block_on(async {
/// use futures::sink::{self, SinkExt};
///
/// let from_fn = sink::from_fn(|i: i32| {
///     async move {
///         eprintln!("{}", i);
///         Ok::<_, futures::never::Never>(())
///     }
/// });
/// futures::pin_mut!(from_fn);
/// from_fn.send(5).await?;
/// # Ok::<(), futures::never::Never>(()) }).unwrap();
/// ```
pub fn from_fn<F, R>(function: F) -> FromFn<F, R> {
    FromFn {
        function,
        future: None,
    }
}

impl<F, R, T, E> Sink<T> for FromFn<F, R>
where
    F: FnMut(T) -> R,
    R: Future<Output = Result<(), E>>,
{
    type Error = E;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.poll_flush(cx)
    }

    fn start_send(self: Pin<&mut Self>, item: T) -> Result<(), Self::Error> {
        let mut this = self.project();
        debug_assert!(this.future.is_none());
        let future = (this.function)(item);
        this.future.set(Some(future));
        Ok(())
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let mut this = self.project();
        if let Some(future) = this.future.as_mut().as_pin_mut() {
            ready!(future.poll(cx))?;
        }
        this.future.set(None);
        Poll::Ready(Ok(()))
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.poll_flush(cx)
    }
}
