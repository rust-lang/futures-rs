use core::marker::Unpin;
use core::pin::Pin;
use futures_core::stream::{FusedStream, Stream, TryStream};
use futures_core::task::{LocalWaker, Poll};
use pin_utils::{unsafe_pinned, unsafe_unpinned};

/// Stream for the [`map_err`](super::TryStreamExt::map_err) combinator.
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
        lw: &LocalWaker,
    ) -> Poll<Option<Self::Item>> {
        match self.stream().try_poll_next(lw) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(opt) =>
                Poll::Ready(opt.map(|res| res.map_err(|e| self.f()(e)))),
        }
    }
}
