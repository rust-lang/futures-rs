use futures_core::future::{FusedFuture, Future};
use futures_core::stream::{FusedStream, Stream};
use futures_core::task::{Context, Poll};
use futures_io::{
    self as io, AsyncBufRead, AsyncRead, AsyncSeek, AsyncWrite, IoSlice, IoSliceMut, SeekFrom,
};
use futures_sink::Sink;
use pin_project::{pin_project, pinned_drop};
use std::pin::Pin;
use std::ptr;
use std::thread::panicking;

/// Combinator that asserts that the underlying type is not moved after being polled.
///
/// See the `assert_unmoved` methods on:
/// * [`FutureTestExt`](crate::future::FutureTestExt::assert_unmoved)
/// * [`StreamTestExt`](crate::stream::StreamTestExt::assert_unmoved)
/// * [`SinkTestExt`](crate::sink::SinkTestExt::assert_unmoved_sink)
/// * [`AsyncReadTestExt`](crate::io::AsyncReadTestExt::assert_unmoved)
/// * [`AsyncWriteTestExt`](crate::io::AsyncWriteTestExt::assert_unmoved_write)
#[pin_project(PinnedDrop, !Unpin)]
#[derive(Debug, Clone)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct AssertUnmoved<T> {
    #[pin]
    inner: T,
    this_ptr: *const Self,
}

// Safety: having a raw pointer in a struct makes it `!Send`, however the
// pointer is never dereferenced so this is safe.
unsafe impl<T: Send> Send for AssertUnmoved<T> {}
unsafe impl<T: Sync> Sync for AssertUnmoved<T> {}

impl<T> AssertUnmoved<T> {
    pub(crate) fn new(inner: T) -> Self {
        Self { inner, this_ptr: ptr::null() }
    }

    fn poll_with<'a, U>(mut self: Pin<&'a mut Self>, f: impl FnOnce(Pin<&'a mut T>) -> U) -> U {
        let cur_this = &*self as *const Self;
        if self.this_ptr.is_null() {
            // First time being polled
            *self.as_mut().project().this_ptr = cur_this;
        } else {
            assert_eq!(self.this_ptr, cur_this, "AssertUnmoved moved between poll calls");
        }
        f(self.project().inner)
    }
}

impl<Fut: Future> Future for AssertUnmoved<Fut> {
    type Output = Fut::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.poll_with(|f| f.poll(cx))
    }
}

impl<Fut: FusedFuture> FusedFuture for AssertUnmoved<Fut> {
    fn is_terminated(&self) -> bool {
        self.inner.is_terminated()
    }
}

impl<St: Stream> Stream for AssertUnmoved<St> {
    type Item = St::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.poll_with(|s| s.poll_next(cx))
    }
}

impl<St: FusedStream> FusedStream for AssertUnmoved<St> {
    fn is_terminated(&self) -> bool {
        self.inner.is_terminated()
    }
}

impl<Si: Sink<Item>, Item> Sink<Item> for AssertUnmoved<Si> {
    type Error = Si::Error;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.poll_with(|s| s.poll_ready(cx))
    }

    fn start_send(self: Pin<&mut Self>, item: Item) -> Result<(), Self::Error> {
        self.poll_with(|s| s.start_send(item))
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.poll_with(|s| s.poll_flush(cx))
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.poll_with(|s| s.poll_close(cx))
    }
}

impl<R: AsyncRead> AsyncRead for AssertUnmoved<R> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        self.poll_with(|r| r.poll_read(cx, buf))
    }

    fn poll_read_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &mut [IoSliceMut<'_>],
    ) -> Poll<io::Result<usize>> {
        self.poll_with(|r| r.poll_read_vectored(cx, bufs))
    }
}

impl<W: AsyncWrite> AsyncWrite for AssertUnmoved<W> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        self.poll_with(|w| w.poll_write(cx, buf))
    }

    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        self.poll_with(|w| w.poll_write_vectored(cx, bufs))
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.poll_with(|w| w.poll_flush(cx))
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.poll_with(|w| w.poll_close(cx))
    }
}

impl<S: AsyncSeek> AsyncSeek for AssertUnmoved<S> {
    fn poll_seek(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        pos: SeekFrom,
    ) -> Poll<io::Result<u64>> {
        self.poll_with(|s| s.poll_seek(cx, pos))
    }
}

impl<R: AsyncBufRead> AsyncBufRead for AssertUnmoved<R> {
    fn poll_fill_buf(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<&[u8]>> {
        self.poll_with(|r| r.poll_fill_buf(cx))
    }

    fn consume(self: Pin<&mut Self>, amt: usize) {
        self.poll_with(|r| r.consume(amt))
    }
}

#[pinned_drop]
impl<T> PinnedDrop for AssertUnmoved<T> {
    fn drop(self: Pin<&mut Self>) {
        // If the thread is panicking then we can't panic again as that will
        // cause the process to be aborted.
        if !panicking() && !self.this_ptr.is_null() {
            let cur_this = &*self as *const Self;
            assert_eq!(self.this_ptr, cur_this, "AssertUnmoved moved before drop");
        }
    }
}

#[cfg(test)]
mod tests {
    use futures_core::future::Future;
    use futures_core::task::{Context, Poll};
    use futures_util::future::pending;
    use futures_util::task::noop_waker;
    use std::pin::Pin;

    use super::AssertUnmoved;

    #[test]
    fn assert_send_sync() {
        fn assert<T: Send + Sync>() {}
        assert::<AssertUnmoved<()>>();
    }

    #[test]
    fn dont_panic_when_not_polled() {
        // This shouldn't panic.
        let future = AssertUnmoved::new(pending::<()>());
        drop(future);
    }

    #[test]
    #[should_panic(expected = "AssertUnmoved moved between poll calls")]
    fn dont_double_panic() {
        // This test should only panic, not abort the process.
        let waker = noop_waker();
        let mut cx = Context::from_waker(&waker);

        // First we allocate the future on the stack and poll it.
        let mut future = AssertUnmoved::new(pending::<()>());
        let pinned_future = unsafe { Pin::new_unchecked(&mut future) };
        assert_eq!(pinned_future.poll(&mut cx), Poll::Pending);

        // Next we move it back to the heap and poll it again. This second call
        // should panic (as the future is moved), but we shouldn't panic again
        // whilst dropping `AssertUnmoved`.
        let mut future = Box::new(future);
        let pinned_boxed_future = unsafe { Pin::new_unchecked(&mut *future) };
        assert_eq!(pinned_boxed_future.poll(&mut cx), Poll::Pending);
    }
}
