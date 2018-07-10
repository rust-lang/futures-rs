use futures_core::future::Future;
use futures_core::task::{self, Poll};
use futures_io::AsyncWrite;
use std::io;
use std::marker::Unpin;
use std::mem::PinMut;

/// A future used to fully close an I/O object.
///
/// Created by the [`close`] function.
///
/// [`close`]: fn.close.html
#[derive(Debug)]
pub struct Close<'a, W: ?Sized + 'a> {
    writer: &'a mut W,
}

// PinMut is never projected to fields
impl<'a, W: ?Sized> Unpin for Close<'a, W> {}

impl<W: AsyncWrite + ?Sized> Close<'a, W> {
    pub(super) fn new(writer: &'a mut W) -> Close<'a, W> {
        Close { writer }
    }
}

impl<'a, W: AsyncWrite + ?Sized> Future for Close<'a, W> {
    type Output = io::Result<()>;

    fn poll(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<Self::Output> {
        self.writer.poll_close(cx)
    }
}
