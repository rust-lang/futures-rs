use core::pin::Pin;
use futures_core::future::Future;
use futures_core::stream::{FusedStream, Stream};
use futures_core::task::{LocalWaker, Poll};
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

impl<St, Fut, F, T> FusedStream for FilterMap<St, Fut, F>
    where St: Stream + FusedStream,
          F: FnMut(St::Item) -> Fut,
          Fut: Future<Output = Option<T>>,
{
    fn is_terminated(&self) -> bool {
        self.pending.is_none() && self.stream.is_terminated()
    }
}

impl<St, Fut, F, T> Stream for FilterMap<St, Fut, F>
    where St: Stream,
          F: FnMut(St::Item) -> Fut,
          Fut: Future<Output = Option<T>>,
{
    type Item = T;

    fn poll_next(
        mut self: Pin<&mut Self>,
        lw: &LocalWaker,
    ) -> Poll<Option<T>> {
        loop {
            if self.as_mut().pending().as_pin_mut().is_none() {
                let item = match ready!(self.as_mut().stream().poll_next(lw)) {
                    Some(e) => e,
                    None => return Poll::Ready(None),
                };
                let fut = (self.as_mut().f())(item);
                self.as_mut().pending().set(Some(fut));
            }

            let item = ready!(self.as_mut().pending().as_pin_mut().unwrap().poll(lw));
            self.as_mut().pending().set(None);
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
