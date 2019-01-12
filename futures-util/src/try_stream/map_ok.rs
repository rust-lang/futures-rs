use core::pin::Pin;
use futures_core::stream::{FusedStream, Stream, TryStream};
use futures_core::task::{LocalWaker, Poll};
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

impl<St: FusedStream, F> FusedStream for MapOk<St, F> {
    fn is_terminated(&self) -> bool {
        self.stream.is_terminated()
    }
}

impl<St, F, T> Stream for MapOk<St, F>
where
    St: TryStream,
    F: FnMut(St::Ok) -> T,
{
    type Item = Result<T, St::Error>;

    #[allow(clippy::redundant_closure)] // https://github.com/rust-lang-nursery/rust-clippy/issues/1439
    fn poll_next(
        mut self: Pin<&mut Self>,
        lw: &LocalWaker,
    ) -> Poll<Option<Self::Item>> {
        match self.as_mut().stream().try_poll_next(lw) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(opt) =>
                Poll::Ready(opt.map(|res| res.map(|x| self.as_mut().f()(x)))),
        }
    }
}
