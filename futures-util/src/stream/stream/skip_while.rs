use core::fmt;
use core::pin::Pin;
use futures_core::future::Future;
use futures_core::iteration;
use futures_core::stream::{FusedStream, Stream};
use futures_core::task::{Context, Poll};
#[cfg(feature = "sink")]
use futures_sink::Sink;
use pin_utils::{unsafe_pinned, unsafe_unpinned};

/// Stream for the [`skip_while`](super::StreamExt::skip_while) method.
#[must_use = "streams do nothing unless polled"]
pub struct SkipWhile<St, Fut, F> where St: Stream {
    stream: St,
    f: F,
    pending_fut: Option<Fut>,
    pending_item: Option<St::Item>,
    done_skipping: bool,
    yield_after: iteration::Limit,
}

impl<St: Unpin + Stream, Fut: Unpin, F> Unpin for SkipWhile<St, Fut, F> {}

impl<St, Fut, F> fmt::Debug for SkipWhile<St, Fut, F>
where
    St: Stream + fmt::Debug,
    St::Item: fmt::Debug,
    Fut: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SkipWhile")
            .field("stream", &self.stream)
            .field("pending_fut", &self.pending_fut)
            .field("pending_item", &self.pending_item)
            .field("done_skipping", &self.done_skipping)
            .field("yield_after", &self.yield_after)
            .finish()
    }
}

impl<St, Fut, F> SkipWhile<St, Fut, F>
    where St: Stream,
          F: FnMut(&St::Item) -> Fut,
          Fut: Future<Output = bool>,
{
    unsafe_pinned!(stream: St);
    unsafe_unpinned!(f: F);
    unsafe_pinned!(pending_fut: Option<Fut>);
    unsafe_unpinned!(pending_item: Option<St::Item>);
    unsafe_unpinned!(done_skipping: bool);
    unsafe_unpinned!(yield_after: iteration::Limit);

    fn split_borrows(
        self: Pin<&mut Self>,
    ) -> (
        Pin<&mut St>,
        &mut F,
        Pin<&mut Option<Fut>>,
        &mut Option<St::Item>,
        &mut bool,
        &mut iteration::Limit,
    ) {
        unsafe {
            let this = self.get_unchecked_mut();
            (
                Pin::new_unchecked(&mut this.stream),
                &mut this.f,
                Pin::new_unchecked(&mut this.pending_fut),
                &mut this.pending_item,
                &mut this.done_skipping,
                &mut this.yield_after,
            )
        }
    }

    pub(super) fn new(stream: St, f: F) -> SkipWhile<St, Fut, F> {
        SkipWhile {
            stream,
            f,
            pending_fut: None,
            pending_item: None,
            done_skipping: false,
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
        #[doc = "the underlying stream and, when pending, a future returned by
            the predicate closure,"]
        #[doc = "items are consecutively yielded by the stream,
            but the predicate immediately resolves to skip them,"]
    }
}

impl<St, Fut, F> FusedStream for SkipWhile<St, Fut, F>
    where St: FusedStream,
          F: FnMut(&St::Item) -> Fut,
          Fut: Future<Output = bool>,
{
    fn is_terminated(&self) -> bool {
        self.pending_item.is_none() && self.stream.is_terminated()
    }
}

impl<St, Fut, F> Stream for SkipWhile<St, Fut, F>
    where St: Stream,
          F: FnMut(&St::Item) -> Fut,
          Fut: Future<Output = bool>,
{
    type Item = St::Item;

    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<St::Item>> {
        if self.done_skipping {
            return self.as_mut().stream().poll_next(cx);
        }

        let (mut stream, predicate, mut pending_fut, pending_item, done_skipping, yield_after) =
            self.split_borrows();
        poll_loop! { yield_after, cx, {
            if pending_item.is_none() {
                let item = match ready!(stream.as_mut().poll_next(cx)) {
                    Some(e) => e,
                    None => return Poll::Ready(None),
                };
                let fut = predicate(&item);
                pending_fut.as_mut().set(Some(fut));
                *pending_item = Some(item);
            }

            let skipped = ready!(pending_fut.as_mut().as_pin_mut().unwrap().poll(cx));
            let item = pending_item.take().unwrap();
            pending_fut.as_mut().set(None);

            if !skipped {
                *done_skipping = true;
                return Poll::Ready(Some(item))
            }
        }}
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let pending_len = if self.pending_item.is_some() { 1 } else { 0 };
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
impl<S, Fut, F, Item> Sink<Item> for SkipWhile<S, Fut, F>
    where S: Stream + Sink<Item>,
          F: FnMut(&S::Item) -> Fut,
          Fut: Future<Output = bool>,
{
    type Error = S::Error;

    delegate_sink!(stream, Item);
}
