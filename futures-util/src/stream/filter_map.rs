use core::mem::PinMut;
use core::marker::Unpin;



use futures_core::{Future, Poll, Stream};
use futures_core::task;

/// A combinator used to filter the results of a stream and simultaneously map
/// them to a different type.
///
/// This structure is returned by the `Stream::filter_map` method.
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct FilterMap<S, R, F>
    where S: Stream,
          F: FnMut(S::Item) -> R,
          R: Future,
{
    stream: S,
    f: F,
    pending: Option<R>,
}

pub fn new<S, R, F>(s: S, f: F) -> FilterMap<S, R, F>
    where S: Stream,
          F: FnMut(S::Item) -> R,
          R: Future,
{
    FilterMap {
        stream: s,
        f: f,
        pending: None,
    }
}

impl<S, R, F> FilterMap<S, R, F>
    where S: Stream,
          F: FnMut(S::Item) -> R,
          R: Future,
{
    /// Acquires a reference to the underlying stream that this combinator is
    /// pulling from.
    pub fn get_ref(&self) -> &S {
        &self.stream
    }

    /// Acquires a mutable reference to the underlying stream that this
    /// combinator is pulling from.
    ///
    /// Note that care must be taken to avoid tampering with the state of the
    /// stream which may otherwise confuse this combinator.
    pub fn get_mut(&mut self) -> &mut S {
        &mut self.stream
    }

    /// Consumes this combinator, returning the underlying stream.
    ///
    /// Note that this may discard intermediate state of this combinator, so
    /// care should be taken to avoid losing resources when this is called.
    pub fn into_inner(self) -> S {
        self.stream
    }

    unsafe_pinned!(stream -> S);
    unsafe_unpinned!(f -> F);
    unsafe_pinned!(pending -> Option<R>);
}

impl<S, R, F> Unpin for FilterMap<S, R, F>
    where S: Stream + Unpin,
          F: FnMut(S::Item) -> R,
          R: Future + Unpin,
{}

/* TODO
// Forwarding impl of Sink from the underlying stream
impl<S, R, F> Sink for FilterMap<S, R, F>
    where S: Stream + Sink,
          F: FnMut(S::Item) -> R,
          R: IntoFuture<Error=S::Error>,
{
    type SinkItem = S::SinkItem;
    type SinkError = S::SinkError;

    delegate_sink!(stream);
}
*/

impl<S, R, F, B> Stream for FilterMap<S, R, F>
    where S: Stream,
          F: FnMut(S::Item) -> R,
          R: Future<Output = Option<B>>,
{
    type Item = B;

    fn poll_next(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<Option<B>> {
        loop {
            if self.pending().as_pin_mut().is_none() {
                let item = match ready!(self.stream().poll_next(cx)) {
                    Some(e) => e,
                    None => return Poll::Ready(None),
                };
                let fut = (self.f())(item);
                PinMut::set(self.pending(), Some(fut));
            }

            let item = ready!(self.pending().as_pin_mut().unwrap().poll(cx));
            PinMut::set(self.pending(), None);
            if item.is_some() {
                return Poll::Ready(item);
            }
        }
    }
}
