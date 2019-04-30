use crate::io::{AsyncSeek, SeekFrom};
use futures_core::future::Future;
use futures_core::task::{Context, Poll};
use std::io;
use std::pin::Pin;

// TODO: Currently this throws a warning, but it's a false positive as it generates a
// correct link. See: https://github.com/rust-lang/rust/issues/55907
// Verify everything works fine here when that get's resolved.
//
/// Future for the [`seek`](AsyncSeekExt::seek) method.
#[derive(Debug)]
pub struct Seek<'a, S: ?Sized + Unpin> {
    seek: &'a mut S,
    pos: SeekFrom,
}

impl<S: ?Sized + Unpin> Unpin for Seek<'_, S> {}

impl<'a, S: AsyncSeek + ?Sized + Unpin> Seek<'a, S> {
    pub(super) fn new(seek: &'a mut S, pos: SeekFrom) -> Self {
        Self { seek, pos }
    }
}

impl<S: AsyncSeek + ?Sized + Unpin> Future for Seek<'_, S> {
    type Output = io::Result<u64>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = &mut *self;
        Pin::new(&mut this.seek).poll_seek(cx, this.pos)
    }
}
