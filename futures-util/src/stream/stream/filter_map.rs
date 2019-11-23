use core::fmt;
use core::pin::Pin;
use futures_core::future::Future;
use futures_core::iteration;
use futures_core::stream::{FusedStream, Stream};
use futures_core::task::{Context, Poll};
#[cfg(feature = "sink")]
use futures_sink::Sink;
use pin_utils::{unsafe_pinned, unsafe_unpinned};

/// Stream for the [`filter_map`](super::StreamExt::filter_map) method.
#[must_use = "streams do nothing unless polled"]
pub struct FilterMap<St, Fut, F> {
    stream: St,
    f: F,
    pending: Option<Fut>,
    yield_after: iteration::Limit,
}

impl<St, Fut, F> Unpin for FilterMap<St, Fut, F>
where
    St: Unpin,
    Fut: Unpin,
{}

impl<St, Fut, F> fmt::Debug for FilterMap<St, Fut, F>
where
    St: fmt::Debug,
    Fut: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FilterMap")
            .field("stream", &self.stream)
            .field("pending", &self.pending)
            .field("yield_after", &self.yield_after)
            .finish()
    }
}

impl<St, Fut, F> FilterMap<St, Fut, F>
    where St: Stream,
          F: FnMut(St::Item) -> Fut,
          Fut: Future,
{
    unsafe_pinned!(stream: St);
    unsafe_unpinned!(f: F);
    unsafe_pinned!(pending: Option<Fut>);
    unsafe_unpinned!(yield_after: iteration::Limit);

    fn split_borrows(
        self: Pin<&mut Self>,
    ) -> (
        Pin<&mut St>,
        &mut F,
        Pin<&mut Option<Fut>>,
        &mut iteration::Limit,
    ) {
        unsafe {
            let this = self.get_unchecked_mut();
            (
                Pin::new_unchecked(&mut this.stream),
                &mut this.f,
                Pin::new_unchecked(&mut this.pending),
                &mut this.yield_after,
            )
        }
    }

    pub(super) fn new(stream: St, f: F) -> FilterMap<St, Fut, F> {
        FilterMap {
            stream,
            f,
            pending: None,
            yield_after: crate::DEFAULT_YIELD_AFTER_LIMIT,
        }
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

    /// Acquires a pinned mutable reference to the underlying stream that this
    /// combinator is pulling from.
    ///
    /// Note that care must be taken to avoid tampering with the state of the
    /// stream which may otherwise confuse this combinator.
    pub fn get_pin_mut(self: Pin<&mut Self>) -> Pin<&mut St> {
        self.stream()
    }

    /// Consumes this combinator, returning the underlying stream.
    ///
    /// Note that this may discard intermediate state of this combinator, so
    /// care should be taken to avoid losing resources when this is called.
    pub fn into_inner(self) -> St {
        self.stream
    }

    stream_method_yield_after_every! {
        #[pollee = "the underlying stream and, when pending, a future returned by the map closure,"]
        #[why_busy = "items are consecutively yielded by the stream,
            but get immediately filtered out,"]
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

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<T>> {
        let (mut stream, op, mut pending, yield_after) = self.split_borrows();
        poll_loop! { yield_after, cx, {
            if pending.as_ref().is_none() {
                let item = match ready!(stream.as_mut().poll_next(cx)) {
                    Some(e) => e,
                    None => return Poll::Ready(None),
                };
                let fut = op(item);
                pending.as_mut().set(Some(fut));
            }

            let item = ready!(pending.as_mut().as_pin_mut().unwrap().poll(cx));
            pending.as_mut().set(None);
            if item.is_some() {
                return Poll::Ready(item);
            }
        }}
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let pending_len = if self.pending.is_some() { 1 } else { 0 };
        let (_, upper) = self.stream.size_hint();
        let upper = match upper {
            Some(x) => x.checked_add(pending_len),
            None => None,
        };
        (0, upper) // can't know a lower bound, due to the predicate
    }
}

// Forwarding impl of Sink from the underlying stream
#[cfg(feature = "sink")]
impl<S, Fut, F, Item> Sink<Item> for FilterMap<S, Fut, F>
    where S: Stream + Sink<Item>,
          F: FnMut(S::Item) -> Fut,
          Fut: Future,
{
    type Error = S::Error;

    delegate_sink!(stream, Item);
}
