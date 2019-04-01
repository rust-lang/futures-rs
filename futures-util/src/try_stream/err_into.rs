use core::marker::PhantomData;
use core::pin::Pin;
use futures_core::stream::{FusedStream, Stream, TryStream};
use futures_core::task::{Waker, Poll};
use futures_sink::Sink;
use pin_utils::unsafe_pinned;

/// Stream for the [`err_into`](super::TryStreamExt::err_into) method.
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct ErrInto<St, E> {
    stream: St,
    _marker: PhantomData<E>,
}

impl<St: Unpin, E> Unpin for ErrInto<St, E> {}

impl<St, E> ErrInto<St, E> {
    unsafe_pinned!(stream: St);

    pub(super) fn new(stream: St) -> Self {
        ErrInto { stream, _marker: PhantomData }
    }
}

impl<St: FusedStream, E> FusedStream for ErrInto<St, E> {
    fn is_terminated(&self) -> bool {
        self.stream.is_terminated()
    }
}

impl<St, E> Stream for ErrInto<St, E>
where
    St: TryStream,
    St::Error: Into<E>,
{
    type Item = Result<St::Ok, E>;

    fn poll_next(
        self: Pin<&mut Self>,
        waker: &Waker,
    ) -> Poll<Option<Self::Item>> {
        self.stream().try_poll_next(waker)
            .map(|res| res.map(|some| some.map_err(Into::into)))
    }
}

// Forwarding impl of Sink from the underlying stream
impl<S, E, Item> Sink<Item> for ErrInto<S, E>
where
    S: TryStream + Sink<Item>,
    S::Error: Into<E>,
{
    type SinkError = S::SinkError;

    delegate_sink!(stream, Item);
}
