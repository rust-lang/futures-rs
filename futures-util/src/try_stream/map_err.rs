use core::pin::Pin;
use futures_core::stream::{FusedStream, Stream, TryStream};
use futures_core::task::{Context, Poll};
use futures_sink::Sink;
use pin_utils::{unsafe_pinned, unsafe_unpinned};

/// Stream for the [`map_err`](super::TryStreamExt::map_err) method.
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct MapErr<St, F> {
    stream: St,
    f: F,
}

impl<St, F> MapErr<St, F> {
    unsafe_pinned!(stream: St);
    unsafe_unpinned!(f: F);

    /// Creates a new MapErr.
    pub(super) fn new(stream: St, f: F) -> Self {
        MapErr { stream, f }
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
    pub fn get_pin_mut<'a>(self: Pin<&'a mut Self>) -> Pin<&'a mut St> {
        self.stream()
    }

    /// Consumes this combinator, returning the underlying stream.
    ///
    /// Note that this may discard intermediate state of this combinator, so
    /// care should be taken to avoid losing resources when this is called.
    pub fn into_inner(self) -> St {
        self.stream
    }
}

impl<St: Unpin, F> Unpin for MapErr<St, F> {}

impl<St: FusedStream, F> FusedStream for MapErr<St, F> {
    fn is_terminated(&self) -> bool {
        self.stream.is_terminated()
    }
}

impl<St, F, E> Stream for MapErr<St, F>
where
    St: TryStream,
    F: FnMut(St::Error) -> E,
{
    type Item = Result<St::Ok, E>;

    #[allow(clippy::redundant_closure)] // https://github.com/rust-lang-nursery/rust-clippy/issues/1439
    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        match self.as_mut().stream().try_poll_next(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(opt) =>
                Poll::Ready(opt.map(|res| res.map_err(|e| self.as_mut().f()(e)))),
        }
    }
}

// Forwarding impl of Sink from the underlying stream
impl<S, F, E, Item> Sink<Item> for MapErr<S, F>
where
    S: TryStream + Sink<Item>,
    F: FnMut(S::Error) -> E,
{
    type SinkError = S::SinkError;

    delegate_sink!(stream, Item);
}
