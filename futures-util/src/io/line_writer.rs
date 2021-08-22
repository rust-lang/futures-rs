use super::buf_writer::BufWriter;
use futures_core::ready;
use futures_core::task::{Context, Poll};
use futures_io::AsyncWrite;
use pin_project_lite::pin_project;
use std::io;
use std::pin::Pin;

pin_project! {
/// Wrap a writer, like [`BufWriter`] does, but prioritizes buffering lines
///
/// This was written based on `std::io::LineWriter` which goes into further details
/// explaining the code.
///
/// Buffering is actually done using `BufWriter`. This class will leverage `BufWriter`
/// to write on-each-line.
#[derive(Debug)]
pub struct LineWriter<W: AsyncWrite> {
    #[pin]
    inner: BufWriter<W>,
}
}

impl<W: AsyncWrite> LineWriter<W> {
    /// Create a new `LineWriter` with default buffer capacity. The default is currently 1KB
    /// which was taken from `std::io::LineWriter`
    pub fn new(inner: W) -> LineWriter<W> {
        LineWriter::with_capacity(1024, inner)
    }

    /// Creates a new `LineWriter` with the specified buffer capacity.
    pub fn with_capacity(capacity: usize, inner: W) -> LineWriter<W> {
        LineWriter { inner: BufWriter::with_capacity(capacity, inner) }
    }

    /// Flush `inner` if last char is "new line"
    fn flush_if_completed_line(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let this = self.project();
        match this.inner.buffer().last().copied() {
            Some(b'\n') => this.inner.flush_buf(cx),
            _ => Poll::Ready(Ok(())),
        }
    }
}

impl<W: AsyncWrite> AsyncWrite for LineWriter<W> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let mut this = self.as_mut().project();
        let newline_index = match memchr::memrchr(b'\n', buf) {
            None => {
                ready!(self.as_mut().flush_if_completed_line(cx)?);
                return self.project().inner.poll_write(cx, buf);
            }
            Some(newline_index) => newline_index + 1,
        };

        ready!(this.inner.as_mut().poll_flush(cx)?);

        let lines = &buf[..newline_index];

        let flushed = ready!(this.inner.as_mut().poll_write(cx, lines))?;

        if flushed == 0 {
            return Poll::Ready(Ok(0));
        }

        let tail = if flushed >= newline_index {
            &buf[flushed..]
        } else if newline_index - flushed <= this.inner.capacity() {
            &buf[flushed..newline_index]
        } else {
            let scan_area = &buf[flushed..];
            let scan_area = &scan_area[..this.inner.capacity()];
            match memchr::memrchr(b'\n', scan_area) {
                Some(newline_index) => &scan_area[..newline_index + 1],
                None => scan_area,
            }
        };

        let buffered = this.inner.write_to_buf(tail);
        Poll::Ready(Ok(flushed + buffered))
    }

    /// Forward to `inner` 's `BufWriter::poll_flush()`
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.as_mut().project().inner.poll_flush(cx)
    }

    /// Forward to `inner` 's `BufWriter::poll_close()`
    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.as_mut().project().inner.poll_close(cx)
    }
}
