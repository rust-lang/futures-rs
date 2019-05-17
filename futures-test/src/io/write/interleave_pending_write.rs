use futures_io::{self as io, AsyncWrite};
use pin_utils::{unsafe_pinned, unsafe_unpinned};
use std::{
    marker::Unpin,
    pin::Pin,
    task::{Context, Poll},
};

/// Writer for the [`interleave_pending_write`](super::AsyncWriteTestExt::interleave_pending_write)
/// method.
#[derive(Debug)]
pub struct InterleavePendingWrite<W: AsyncWrite> {
    writer: W,
    pended: bool,
}

impl<W: AsyncWrite + Unpin> Unpin for InterleavePendingWrite<W> {}

impl<W: AsyncWrite> InterleavePendingWrite<W> {
    unsafe_pinned!(writer: W);
    unsafe_unpinned!(pended: bool);

    pub(crate) fn new(writer: W) -> Self {
        Self {
            writer,
            pended: false,
        }
    }

    /// Acquires a reference to the underlying writer that this adaptor is wrapping.
    pub fn get_ref(&self) -> &W {
        &self.writer
    }

    /// Acquires a mutable reference to the underlying writer that this adaptor is wrapping.
    pub fn get_mut(&mut self) -> &mut W {
        &mut self.writer
    }

    /// Acquires a pinned mutable reference to the underlying writer that this adaptor is wrapping.
    pub fn get_pin_mut<'a>(self: Pin<&'a mut Self>) -> Pin<&'a mut W> {
        self.project().0
    }

    /// Consumes this adaptor returning the underlying writer.
    pub fn into_inner(self) -> W {
        self.writer
    }

    fn project<'a>(self: Pin<&'a mut Self>) -> (Pin<&'a mut W>, &'a mut bool) {
        unsafe {
            let this = self.get_unchecked_mut();
            (Pin::new_unchecked(&mut this.writer), &mut this.pended)
        }
    }
}

impl<W: AsyncWrite> AsyncWrite for InterleavePendingWrite<W> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let (writer, pended) = self.project();
        if *pended {
            let next = writer.poll_write(cx, buf);
            if next.is_ready() {
                *pended = false;
            }
            next
        } else {
            cx.waker().wake_by_ref();
            *pended = true;
            Poll::Pending
        }
    }

    fn poll_flush(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<io::Result<()>> {
        let (writer, pended) = self.project();
        if *pended {
            let next = writer.poll_flush(cx);
            if next.is_ready() {
                *pended = false;
            }
            next
        } else {
            cx.waker().wake_by_ref();
            *pended = true;
            Poll::Pending
        }
    }

    fn poll_close(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<io::Result<()>> {
        let (writer, pended) = self.project();
        if *pended {
            let next = writer.poll_close(cx);
            if next.is_ready() {
                *pended = false;
            }
            next
        } else {
            cx.waker().wake_by_ref();
            *pended = true;
            Poll::Pending
        }
    }
}
