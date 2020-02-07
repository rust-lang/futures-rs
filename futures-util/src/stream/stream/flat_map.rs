use super::Map;
use core::fmt;
use core::pin::Pin;
use futures_core::stream::{FusedStream, Stream};
use futures_core::task::{Context, Poll};
#[cfg(feature = "sink")]
use futures_sink::Sink;
use pin_utils::unsafe_pinned;

/// Stream for the [`flat_map`](super::StreamExt::flat_map) method.
#[must_use = "streams do nothing unless polled"]
pub struct FlatMap<St, U, F> {
    stream: Map<St, F>,
    inner_stream: Option<U>,
}

impl<St: Unpin, U, F> Unpin for FlatMap<St, U, F> {}

impl<St, U, F> fmt::Debug for FlatMap<St, U, F>
where
    St: fmt::Debug,
    U: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FlatMap")
            .field("stream", &self.stream)
            .field("inner_stream", &self.inner_stream)
            .finish()
    }
}

impl<St, U, F> FlatMap<St, U, F>
where
    St: Stream,
    U: Stream,
    F: FnMut(St::Item) -> U,
{
    unsafe_pinned!(stream: Map<St, F>);
    unsafe_pinned!(inner_stream: Option<U>);

    pub(super) fn new(stream: St, f: F) -> FlatMap<St, U, F> {
        FlatMap {
            stream: Map::new(stream, f),
            inner_stream: None,
        }
    }

    /// Acquires a reference to the underlying stream that this combinator is
    /// pulling from.
    pub fn get_ref(&self) -> &St {
        self.stream.get_ref()
    }

    /// Acquires a mutable reference to the underlying stream that this
    /// combinator is pulling from.
    ///
    /// Note that care must be taken to avoid tampering with the state of the
    /// stream which may otherwise confuse this combinator.
    pub fn get_mut(&mut self) -> &mut St {
        self.stream.get_mut()
    }

    /// Acquires a pinned mutable reference to the underlying stream that this
    /// combinator is pulling from.
    ///
    /// Note that care must be taken to avoid tampering with the state of the
    /// stream which may otherwise confuse this combinator.
    pub fn get_pin_mut(self: Pin<&mut Self>) -> Pin<&mut St> {
        self.stream().get_pin_mut()
    }

    /// Consumes this combinator, returning the underlying stream.
    ///
    /// Note that this may discard intermediate state of this combinator, so
    /// care should be taken to avoid losing resources when this is called.
    pub fn into_inner(self) -> St {
        self.stream.into_inner()
    }
}

impl<St, F, U> FusedStream for FlatMap<St, U, F>
where
    St: FusedStream,
    U: FusedStream,
    F: FnMut(St::Item) -> U,
{
    fn is_terminated(&self) -> bool {
        self.stream.is_terminated()
            && self
                .inner_stream
                .as_ref()
                .map(FusedStream::is_terminated)
                .unwrap_or(true)
    }
}

impl<St, F, U> Stream for FlatMap<St, U, F>
where
    St: Stream,
    U: Stream,
    F: FnMut(St::Item) -> U,
{
    type Item = U::Item;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            if let Some(inner_stream) = self.as_mut().inner_stream().as_pin_mut() {
                let next = ready!(inner_stream.poll_next(cx));
                if next.is_some() {
                    break Poll::Ready(next);
                } else {
                    self.as_mut().inner_stream().set(None);
                }
            }

            let next_stream = ready!(self.as_mut().stream().poll_next(cx));

            if next_stream.is_some() {
                self.as_mut().inner_stream().set(next_stream);
            } else {
                break Poll::Ready(None);
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let stream_size_hint = self.stream.size_hint();
        let no_stream_items_left = stream_size_hint.1 == Some(0);

        if let Some(inner_stream_size_hint) = self.inner_stream.as_ref().map(|st| st.size_hint()) {
            (
                stream_size_hint.0 + inner_stream_size_hint.0,
                if no_stream_items_left {
                    inner_stream_size_hint.1
                } else {
                    // Can't know upper bound because next items are `Stream`s
                    None
                },
            )
        } else {
            (
                stream_size_hint.0,
                if no_stream_items_left {
                    Some(0)
                } else {
                    // Can't know upper bound because next items are `Stream`s
                    None
                },
            )
        }
    }
}

// Forwarding impl of Sink from the underlying stream
#[cfg(feature = "sink")]
impl<S, F, U, Item> Sink<Item> for FlatMap<S, U, F>
where
    S: Stream + Sink<Item>,
    U: Stream,
    F: FnMut(S::Item) -> U,
{
    type Error = S::Error;

    delegate_sink!(stream, Item);
}
