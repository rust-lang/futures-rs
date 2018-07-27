use core::marker::{PhantomData, Unpin};
use core::mem::PinMut;
use futures_core::stream::{Stream, TryStream};
use futures_core::task::{self, Poll};

/// Stream for the [`err_into`](super::TryStreamExt::err_into) combinator.
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

impl<St, E> Stream for ErrInto<St, E>
where
    St: TryStream,
    St::Error: Into<E>,
{
    type Item = Result<St::Ok, E>;

    fn poll_next(
        mut self: PinMut<Self>,
        cx: &mut task::Context,
    ) -> Poll<Option<Self::Item>> {
        self.stream().try_poll_next(cx)
            .map(|res| res.map(|some| some.map_err(Into::into)))
    }
}
