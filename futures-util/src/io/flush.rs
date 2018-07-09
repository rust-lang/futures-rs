use futures_core::future::Future;
use futures_core::task::{self, Poll};
use std::io;
use std::marker::Unpin;
use std::mem::PinMut;

use futures_io::AsyncWrite;

/// A future used to fully flush an I/O object.
///
/// Resolves to the underlying I/O object once the flush operation is complete.
///
/// Created by the [`flush`] function.
///
/// [`flush`]: fn.flush.html
#[derive(Debug)]
pub struct Flush<'a, A: ?Sized + 'a> {
    a: &'a mut A,
}

// Pinning is never projected to fields
impl<'a, A: ?Sized> Unpin for Flush<'a, A> {}

pub fn flush<'a, A>(a: &'a mut A) -> Flush<'a, A>
    where A: AsyncWrite + ?Sized,
{
    Flush { a }
}

impl<'a, A> Future for Flush<'a, A>
    where A: AsyncWrite + ?Sized,
{
    type Output = io::Result<()>;

    fn poll(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<Self::Output> {
        self.a.poll_flush(cx)
    }
}
