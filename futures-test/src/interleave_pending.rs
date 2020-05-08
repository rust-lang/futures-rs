use futures_core::future::{Future, FusedFuture};
use futures_core::stream::{Stream, FusedStream};
use futures_io::{self as io, AsyncBufRead, AsyncRead, AsyncWrite};
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
        Self {
            inner,
            pended: false,
        }
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
}

impl<Fut: Future> Future for InterleavePending<Fut> {
    type Output = Fut::Output;

    fn poll(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Self::Output> {
        let this = self.project();
        if *this.pended {
            let next = this.inner.poll(cx);
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

impl<Fut: FusedFuture> FusedFuture for InterleavePending<Fut> {
    fn is_terminated(&self) -> bool {
        self.inner.is_terminated()
    }
}

impl<St: Stream> Stream for InterleavePending<St> {
    type Item = St::Item;

    fn poll_next(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        let this = self.project();
        if *this.pended {
            let next = this.inner.poll_next(cx);
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

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl<Fut: FusedStream> FusedStream for InterleavePending<Fut> {
    fn is_terminated(&self) -> bool {
        self.inner.is_terminated()
    }
}

impl<W: AsyncWrite> AsyncWrite for InterleavePending<W> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let this = self.project();
        if *this.pended {
            let next = this.inner.poll_write(cx, buf);
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

    fn poll_flush(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<io::Result<()>> {
        let this = self.project();
        if *this.pended {
            let next = this.inner.poll_flush(cx);
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

    fn poll_close(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<io::Result<()>> {
        let this = self.project();
        if *this.pended {
            let next = this.inner.poll_close(cx);
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

impl<R: AsyncRead> AsyncRead for InterleavePending<R> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        let this = self.project();
        if *this.pended {
            let next = this.inner.poll_read(cx, buf);
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

impl<R: AsyncBufRead> AsyncBufRead for InterleavePending<R> {
    fn poll_fill_buf(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<io::Result<&[u8]>> {
        let this = self.project();
        if *this.pended {
            let next = this.inner.poll_fill_buf(cx);
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

    fn consume(self: Pin<&mut Self>, amount: usize) {
        self.project().inner.consume(amount)
    }
}
