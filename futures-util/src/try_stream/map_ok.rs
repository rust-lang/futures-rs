use core::marker::Unpin;
use core::pin::PinMut;
use futures_core::stream::{Stream, TryStream};
use futures_core::task::{self, Poll};
use pin_utils::{unsafe_pinned, unsafe_unpinned};

/// Stream for the [`map_ok`](super::TryStreamExt::map_ok) combinator.
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct MapOk<St, F> {
    stream: St,
    f: F,
}

impl<St, F> MapOk<St, F> {
    unsafe_pinned!(stream: St);
    unsafe_unpinned!(f: F);

    /// Creates a new MapOk.
    pub(super) fn new(stream: St, f: F) -> Self {
        MapOk { stream, f }
    }
}

impl<St: Unpin, F> Unpin for MapOk<St, F> {}

impl<St, F, T> Stream for MapOk<St, F>
where
    St: TryStream,
    F: FnMut(St::Ok) -> T,
{
    type Item = Result<T, St::Error>;

    #[allow(clippy::redundant_closure)] // https://github.com/rust-lang-nursery/rust-clippy/issues/1439
    fn poll_next(
        mut self: PinMut<Self>,
        cx: &mut task::Context,
    ) -> Poll<Option<Self::Item>> {
        match self.stream().try_poll_next(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(opt) =>
                Poll::Ready(opt.map(|res| res.map(|x| self.f()(x)))),
        }
    }
}
