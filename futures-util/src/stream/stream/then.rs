use core::fmt;
use core::pin::Pin;
use futures_core::future::Future;
use futures_core::stream::{FusedStream, Stream};
use futures_core::task::{Context, Poll};
#[cfg(feature = "sink")]
use futures_sink::Sink;
use pin_project::{pin_project, project};

/// Stream for the [`then`](super::StreamExt::then) method.
#[pin_project]
#[must_use = "streams do nothing unless polled"]
pub struct Then<St, Fut, F> {
    #[pin]
    stream: St,
    #[pin]
    future: Option<Fut>,
    f: F,
}

impl<St, Fut, F> fmt::Debug for Then<St, Fut, F>
where
    St: fmt::Debug,
    Fut: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Then")
            .field("stream", &self.stream)
            .field("future", &self.future)
            .finish()
    }
}

impl<St, Fut, F> Then<St, Fut, F>
    where St: Stream,
          F: FnMut(St::Item) -> Fut,
{
    pub(super) fn new(stream: St, f: F) -> Then<St, Fut, F> {
        Then {
            stream,
            future: None,
            f,
        }
    }

    delegate_access_inner!(stream, St, ());
}

impl<St, Fut, F> FusedStream for Then<St, Fut, F>
    where St: FusedStream,
          F: FnMut(St::Item) -> Fut,
          Fut: Future,
{
    fn is_terminated(&self) -> bool {
        self.future.is_none() && self.stream.is_terminated()
    }
}

impl<St, Fut, F> Stream for Then<St, Fut, F>
    where St: Stream,
          F: FnMut(St::Item) -> Fut,
          Fut: Future,
{
    type Item = Fut::Output;

    #[project]
    fn poll_next(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Fut::Output>> {
        #[project]
        let Then { mut stream, f, mut future } = self.project();

        Poll::Ready(loop {
            if let Some(fut) = future.as_mut().as_pin_mut() {
                let item = ready!(fut.poll(cx));
                future.set(None);
                break Some(item);
            } else if let Some(item) = ready!(stream.as_mut().poll_next(cx)) {
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

// Forwarding impl of Sink from the underlying stream
#[cfg(feature = "sink")]
impl<S, Fut, F, Item> Sink<Item> for Then<S, Fut, F>
    where S: Sink<Item>,
{
    type Error = S::Error;

    delegate_sink!(stream, Item);
}
