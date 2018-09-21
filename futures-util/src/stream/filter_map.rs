use core::marker::Unpin;
use core::pin::PinMut;
use futures_core::future::Future;
use futures_core::stream::Stream;
use futures_core::task::{self, Poll};
use pin_utils::{unsafe_pinned, unsafe_unpinned};

/// A combinator used to filter the results of a stream and simultaneously map
/// them to a different type.
///
/// This structure is returned by the `Stream::filter_map` method.
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct FilterMap<St, Fut, F>
    where St: Stream,
          F: FnMut(St::Item) -> Fut,
          Fut: Future,
{
    stream: St,
    f: F,
    pending: Option<Fut>,
}

impl<St, Fut, F> Unpin for FilterMap<St, Fut, F>
    where St: Stream + Unpin,
          F: FnMut(St::Item) -> Fut,
          Fut: Future + Unpin,
{}

impl<St, Fut, F> FilterMap<St, Fut, F>
    where St: Stream,
          F: FnMut(St::Item) -> Fut,
          Fut: Future,
{
    unsafe_pinned!(stream: St);
    unsafe_unpinned!(f: F);
    unsafe_pinned!(pending: Option<Fut>);

    pub(super) fn new(stream: St, f: F) -> FilterMap<St, Fut, F> {
        FilterMap { stream, f, pending: None }
    }

    /// Acquires a reference to the underlying stream that this combinator is
    /// pulling from.
    pub fn get_ref(&self) -> &St {
        &self.stream
    }

    /// Acquires a mutable reference to the underlying stream that this
    /// combinator is pulling from.
    ///
    /// Note that care must be taken to avoid tampering with the state of the
    /// stream which may otherwise confuse this combinator.
    pub fn get_mut(&mut self) -> &mut St {
        &mut self.stream
    }

    /// Consumes this combinator, returning the underlying stream.
    ///
    /// Note that this may discard intermediate state of this combinator, so
    /// care should be taken to avoid losing resources when this is called.
    pub fn into_inner(self) -> St {
        self.stream
    }
}

impl<St, Fut, F, T> Stream for FilterMap<St, Fut, F>
    where St: Stream,
          F: FnMut(St::Item) -> Fut,
          Fut: Future<Output = Option<T>>,
{
    type Item = T;

    fn poll_next(
        mut self: PinMut<Self>,
        cx: &mut task::Context,
    ) -> Poll<Option<T>> {
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
