use core::marker::Unpin;
use core::mem::PinMut;
use futures_core::future::Future;
use futures_core::stream::Stream;
use futures_core::task::{self, Poll};
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

impl<St, Fut, F> Stream for Then<St, Fut, F>
    where St: Stream,
          F: FnMut(St::Item) -> Fut,
          Fut: Future,
{
    type Item = Fut::Output;

    fn poll_next(
        mut self: PinMut<Self>,
        cx: &mut task::Context
    ) -> Poll<Option<Fut::Output>> {
        if self.future().as_pin_mut().is_none() {
            let item = match ready!(self.stream().poll_next(cx)) {
                None => return Poll::Ready(None),
                Some(e) => e,
            };
            let fut = (self.f())(item);
            PinMut::set(self.future(), Some(fut));
        }

        let e = ready!(self.future().as_pin_mut().unwrap().poll(cx));
        PinMut::set(self.future(), None);
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
