use core::mem::PinMut;
use core::marker::Unpin;



use futures_core::{Future, Poll, Stream};
use futures_core::task;

/// A stream combinator which chains a computation onto each item produced by a
/// stream.
///
/// This structure is produced by the `Stream::then` method.
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct Then<S, U, F> {
    stream: S,
    future: Option<U>,
    f: F,
}

pub fn new<S, U, F>(s: S, f: F) -> Then<S, U, F>
    where S: Stream,
          F: FnMut(S::Item) -> U,
{
    Then {
        stream: s,
        future: None,
        f,
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

impl<S, U, F> Then<S, U, F> {
    unsafe_pinned!(stream -> S);
    unsafe_pinned!(future -> Option<U>);
    unsafe_unpinned!(f -> F);
}

impl<S: Unpin, U: Unpin, F> Unpin for Then<S, U, F> {}

impl<S, U, F> Stream for Then<S, U, F>
    where S: Stream,
          F: FnMut(S::Item) -> U,
          U: Future,
{
    type Item = U::Output;

    fn poll_next(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<Option<U::Output>> {
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
