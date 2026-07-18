use core::pin::Pin;
use std::io;

use futures_core::{
    future::Future,
    task::{Context, Poll},
};

use crate::io::{AsyncSeek, SeekFrom};

/// Future for the [`seek`](crate::io::AsyncSeekExt::seek) method.
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct Seek<'a, S: ?Sized> {
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
