use futures_io::{self as io, AsyncBufRead, AsyncRead, AsyncWrite};
use pin_utils::{unsafe_pinned, unsafe_unpinned};
use std::{
    pin::Pin,
    task::{Context, Poll},
};

/// I/O wrapper for interleaving [`Poll::Pending`] in calls to read or write.
///
/// See the [`interleave_pending`] and [`interleave_pending_write`] methods.
///
/// [`interleave_pending`]: super::AsyncReadTestExt::interleave_pending
/// [`interleave_pending_write`]: super::AsyncWriteTestExt::interleave_pending_write
#[derive(Debug)]
pub struct InterleavePending<IO> {
    io: IO,
    pended: bool,
}

impl<IO: Unpin> Unpin for InterleavePending<IO> {}

impl<IO> InterleavePending<IO> {
    unsafe_pinned!(io: IO);
    unsafe_unpinned!(pended: bool);

    pub(crate) fn new(io: IO) -> Self {
        Self { io, pended: false }
    }

    /// Acquires a reference to the underlying I/O object that this adaptor is
    /// wrapping.
    pub fn get_ref(&self) -> &IO {
        &self.io
    }

    /// Acquires a mutable reference to the underlying I/O object that this
    /// adaptor is wrapping.
    pub fn get_mut(&mut self) -> &mut IO {
        &mut self.io
    }

    /// Acquires a pinned mutable reference to the underlying I/O object that
    /// this adaptor is wrapping.
    pub fn get_pin_mut<'a>(self: Pin<&'a mut Self>) -> Pin<&'a mut IO> {
        self.project().0
    }

    /// Consumes this adaptor returning the underlying I/O object.
    pub fn into_inner(self) -> IO {
        self.io
    }

    fn project<'a>(self: Pin<&'a mut Self>) -> (Pin<&'a mut IO>, &'a mut bool) {
        unsafe {
            let this = self.get_unchecked_mut();
            (Pin::new_unchecked(&mut this.io), &mut this.pended)
        }
    }
}

impl<W: AsyncWrite> AsyncWrite for InterleavePending<W> {
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

impl<R: AsyncRead> AsyncRead for InterleavePending<R> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        let (reader, pended) = self.project();
        if *pended {
            let next = reader.poll_read(cx, buf);
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

impl<R: AsyncBufRead> AsyncBufRead for InterleavePending<R> {
    fn poll_fill_buf<'a>(
        self: Pin<&'a mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<io::Result<&'a [u8]>> {
        let (reader, pended) = self.project();
        if *pended {
            let next = reader.poll_fill_buf(cx);
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

    fn consume(self: Pin<&mut Self>, amount: usize) {
        self.io().consume(amount)
    }
}
