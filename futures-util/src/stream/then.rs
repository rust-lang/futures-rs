use core::marker::Unpin;
use core::pin::Pin;
use futures_core::future::Future;
use futures_core::stream::{FusedStream, Stream};
use futures_core::task::{LocalWaker, Poll};
use pin_utils::{unsafe_pinned, unsafe_unpinned};

/// A stream combinator which chains a computation onto each item produced by a
/// stream.
///
/// This structure is produced by the `Stream::then` method.
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct Then<St, Fut, F> {
    stream: St,
    future: Option<Fut>,
    f: F,
}

impl<St: Unpin, Fut: Unpin, F> Unpin for Then<St, Fut, F> {}

impl<St, Fut, F> Then<St, Fut, F>
    where St: Stream,
          F: FnMut(St::Item) -> Fut,
{
    unsafe_pinned!(stream: St);
    unsafe_pinned!(future: Option<Fut>);
    unsafe_unpinned!(f: F);

    pub(super) fn new(stream: St, f: F) -> Then<St, Fut, F> {
        Then {
            stream,
            future: None,
            f,
        }
    }
}

impl<St: FusedStream, Fut, F> FusedStream for Then<St, Fut, F> {
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

    fn poll_next(
        mut self: Pin<&mut Self>,
        lw: &LocalWaker
    ) -> Poll<Option<Fut::Output>> {
        if self.future().as_pin_mut().is_none() {
            let item = match ready!(self.stream().poll_next(lw)) {
                None => return Poll::Ready(None),
                Some(e) => e,
            };
            let fut = (self.f())(item);
            Pin::set(self.future(), Some(fut));
        }

        let e = ready!(self.future().as_pin_mut().unwrap().poll(lw));
        Pin::set(self.future(), None);
        Poll::Ready(Some(e))
    }
}

/* TODO
// Forwarding impl of Sink from the underlying stream
impl<S, U, F> Sink for Then<S, U, F>
    where S: Sink, U: IntoFuture,
{
    type SinkItem = S::SinkItem;
    type SinkError = S::SinkError;

    delegate_sink!(stream);
}
 */
