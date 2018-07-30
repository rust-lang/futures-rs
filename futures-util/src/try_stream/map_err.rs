use core::marker::Unpin;
use core::mem::PinMut;
use futures_core::stream::{Stream, TryStream};
use futures_core::task::{self, Poll};

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

impl<St, F, E> Stream for MapErr<St, F>
where
    St: TryStream,
    F: FnMut(St::Error) -> E,
{
    type Item = Result<St::Ok, E>;

    #[allow(redundant_closure)] // https://github.com/rust-lang-nursery/rust-clippy/issues/1439
    fn poll_next(
        mut self: PinMut<Self>,
        cx: &mut task::Context,
    ) -> Poll<Option<Self::Item>> {
        match self.stream().try_poll_next(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(opt) =>
                Poll::Ready(opt.map(|res| res.map_err(|e| self.f()(e)))),
        }
    }
}
