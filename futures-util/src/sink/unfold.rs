use core::{future::Future, pin::Pin};
use futures_core::task::{Context, Poll};
use futures_sink::Sink;
use pin_project::pin_project;

/// Sink for the [`unfold`] function.
#[pin_project]
#[derive(Debug)]
#[must_use = "sinks do nothing unless polled"]
pub struct Unfold<T, F, R> {
    state: Option<T>,
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
/// let unfold = sink::unfold(0, |mut sum, i: i32| {
///     async move {
///         sum += i;
///         eprintln!("{}", i);
///         Ok::<_, futures::never::Never>(sum)
///     }
/// });
/// futures::pin_mut!(unfold);
/// unfold.send(5).await?;
/// # Ok::<(), futures::never::Never>(()) }).unwrap();
/// ```
pub fn unfold<T, F, R>(init: T, function: F) -> Unfold<T, F, R> {
    Unfold {
        state: Some(init),
        function,
        future: None,
    }
}

impl<T, F, R, Item, E> Sink<Item> for Unfold<T, F, R>
where
    F: FnMut(T, Item) -> R,
    R: Future<Output = Result<T, E>>,
{
    type Error = E;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.poll_flush(cx)
    }

    fn start_send(self: Pin<&mut Self>, item: Item) -> Result<(), Self::Error> {
        let mut this = self.project();
        debug_assert!(this.future.is_none());
        let future = (this.function)(this.state.take().unwrap(), item);
        this.future.set(Some(future));
        Ok(())
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let mut this = self.project();
        Poll::Ready(if let Some(future) = this.future.as_mut().as_pin_mut() {
            let result = match ready!(future.poll(cx)) {
                Ok(state) => {
                    *this.state = Some(state);
                    Ok(())
                }
                Err(err) => Err(err),
            };
            this.future.set(None);
            result
        } else {
            Ok(())
        })
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.poll_flush(cx)
    }
}
