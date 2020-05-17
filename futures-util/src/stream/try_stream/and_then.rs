use core::fmt;
use core::pin::Pin;
use futures_core::future::TryFuture;
use futures_core::stream::{Stream, TryStream, FusedStream};
use futures_core::task::{Context, Poll};
#[cfg(feature = "sink")]
use futures_sink::Sink;
use pin_project::{pin_project, project};

/// Stream for the [`and_then`](super::TryStreamExt::and_then) method.
#[pin_project]
#[must_use = "streams do nothing unless polled"]
pub struct AndThen<St, Fut, F> {
    #[pin]
    stream: St,
    #[pin]
    future: Option<Fut>,
    f: F,
}

impl<St, Fut, F> fmt::Debug for AndThen<St, Fut, F>
where
    St: fmt::Debug,
    Fut: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AndThen")
            .field("stream", &self.stream)
            .field("future", &self.future)
            .finish()
    }
}

impl<St, Fut, F> AndThen<St, Fut, F>
    where St: TryStream,
          F: FnMut(St::Ok) -> Fut,
          Fut: TryFuture<Error = St::Error>,
{
    pub(super) fn new(stream: St, f: F) -> Self {
        Self { stream, future: None, f }
    }

    delegate_access_inner!(stream, St, ());
}

impl<St, Fut, F> Stream for AndThen<St, Fut, F>
    where St: TryStream,
          F: FnMut(St::Ok) -> Fut,
          Fut: TryFuture<Error = St::Error>,
{
    type Item = Result<Fut::Ok, St::Error>;

    #[project]
    fn poll_next(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        #[project]
        let AndThen { mut stream, mut future, f } = self.project();

        Poll::Ready(loop {
            if let Some(fut) = future.as_mut().as_pin_mut() {
                let item = ready!(fut.try_poll(cx));
                future.set(None);
                break Some(item);
            } else if let Some(item) = ready!(stream.as_mut().try_poll_next(cx)?) {
                future.set(Some(f(item)));
            } else {
                break None;
            }
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let future_len = if self.future.is_some() { 1 } else { 0 };
        let (lower, upper) = self.stream.size_hint();
        let lower = lower.saturating_add(future_len);
        let upper = match upper {
            Some(x) => x.checked_add(future_len),
            None => None,
        };
        (lower, upper)
    }
}

impl<St, Fut, F> FusedStream for AndThen<St, Fut, F>
    where St: TryStream + FusedStream,
          F: FnMut(St::Ok) -> Fut,
          Fut: TryFuture<Error = St::Error>,
{
    fn is_terminated(&self) -> bool {
        self.future.is_none() && self.stream.is_terminated()
    }
}

// Forwarding impl of Sink from the underlying stream
#[cfg(feature = "sink")]
impl<S, Fut, F, Item> Sink<Item> for AndThen<S, Fut, F>
    where S: Sink<Item>,
{
    type Error = S::Error;

    delegate_sink!(stream, Item);
}
