use futures_core::future::{FusedFuture, Future};
use futures_core::stream::{FusedStream, Stream};
use futures_io::{
    self as io, AsyncBufRead, AsyncRead, AsyncSeek, AsyncWrite, IoSlice, IoSliceMut, SeekFrom,
};
use futures_sink::Sink;
use pin_project::pin_project;
use std::{
    pin::Pin,
    task::{Context, Poll},
};

/// Wrapper that interleaves [`Poll::Pending`] in calls to poll.
///
/// See the `interleave_pending` methods on:
/// * [`FutureTestExt`](crate::future::FutureTestExt::interleave_pending)
/// * [`StreamTestExt`](crate::stream::StreamTestExt::interleave_pending)
/// * [`SinkTestExt`](crate::sink::SinkTestExt::interleave_pending_sink)
/// * [`AsyncReadTestExt`](crate::io::AsyncReadTestExt::interleave_pending)
/// * [`AsyncWriteTestExt`](crate::io::AsyncWriteTestExt::interleave_pending_write)
#[pin_project]
#[derive(Debug)]
pub struct InterleavePending<T> {
    #[pin]
    inner: T,
    pended: bool,
}

impl<T> InterleavePending<T> {
    pub(crate) fn new(inner: T) -> Self {
        Self { inner, pended: false }
    }

    /// Acquires a reference to the underlying I/O object that this adaptor is
    /// wrapping.
    pub fn get_ref(&self) -> &T {
        &self.inner
    }

    /// Acquires a mutable reference to the underlying I/O object that this
    /// adaptor is wrapping.
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.inner
    }

    /// Acquires a pinned mutable reference to the underlying I/O object that
    /// this adaptor is wrapping.
    pub fn get_pin_mut(self: Pin<&mut Self>) -> Pin<&mut T> {
        self.project().inner
    }

    /// Consumes this adaptor returning the underlying I/O object.
    pub fn into_inner(self) -> T {
        self.inner
    }

    fn poll_with<'a, U>(
        self: Pin<&'a mut Self>,
        cx: &mut Context<'_>,
        f: impl FnOnce(Pin<&'a mut T>, &mut Context<'_>) -> Poll<U>,
    ) -> Poll<U> {
        let this = self.project();
        if *this.pended {
            let next = f(this.inner, cx);
            if next.is_ready() {
                *this.pended = false;
            }
            next
        } else {
            cx.waker().wake_by_ref();
            *this.pended = true;
            Poll::Pending
        }
    }
}

impl<Fut: Future> Future for InterleavePending<Fut> {
    type Output = Fut::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.poll_with(cx, Fut::poll)
    }
}

impl<Fut: FusedFuture> FusedFuture for InterleavePending<Fut> {
    fn is_terminated(&self) -> bool {
        self.inner.is_terminated()
    }
}

impl<St: Stream> Stream for InterleavePending<St> {
    type Item = St::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.poll_with(cx, St::poll_next)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl<St: FusedStream> FusedStream for InterleavePending<St> {
    fn is_terminated(&self) -> bool {
        self.inner.is_terminated()
    }
}

impl<Si: Sink<Item>, Item> Sink<Item> for InterleavePending<Si> {
    type Error = Si::Error;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.poll_with(cx, Si::poll_ready)
    }

    fn start_send(self: Pin<&mut Self>, item: Item) -> Result<(), Self::Error> {
        self.project().inner.start_send(item)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.poll_with(cx, Si::poll_flush)
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.poll_with(cx, Si::poll_close)
    }
}

impl<R: AsyncRead> AsyncRead for InterleavePending<R> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        self.poll_with(cx, |r, cx| r.poll_read(cx, buf))
    }

    fn poll_read_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &mut [IoSliceMut<'_>],
    ) -> Poll<io::Result<usize>> {
        self.poll_with(cx, |r, cx| r.poll_read_vectored(cx, bufs))
    }
}

impl<W: AsyncWrite> AsyncWrite for InterleavePending<W> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        self.poll_with(cx, |w, cx| w.poll_write(cx, buf))
    }

    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        self.poll_with(cx, |w, cx| w.poll_write_vectored(cx, bufs))
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.poll_with(cx, W::poll_flush)
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.poll_with(cx, W::poll_close)
    }
}

impl<S: AsyncSeek> AsyncSeek for InterleavePending<S> {
    fn poll_seek(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        pos: SeekFrom,
    ) -> Poll<io::Result<u64>> {
        self.poll_with(cx, |s, cx| s.poll_seek(cx, pos))
    }
}

impl<R: AsyncBufRead> AsyncBufRead for InterleavePending<R> {
    fn poll_fill_buf(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<&[u8]>> {
        self.poll_with(cx, R::poll_fill_buf)
    }

    fn consume(self: Pin<&mut Self>, amount: usize) {
        self.project().inner.consume(amount)
    }
}
