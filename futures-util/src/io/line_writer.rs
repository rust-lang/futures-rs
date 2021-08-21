use super::buf_writer::BufWriter;
use futures_core::ready;
use futures_core::task::{Context, Poll};
use futures_io::AsyncWrite;
use pin_project_lite::pin_project;
use pin_utils::pin_mut;
use std::io;
use std::pin::Pin;

pin_project! {
#[derive(Debug)]
struct LineWriterShim<W: AsyncWrite> {
    #[pin]
    buffer: BufWriter<W>, // TODO HELP what's this field's type suppossed to be?
}
}

impl<W: AsyncWrite> LineWriterShim<W> {
    /// TODO WIP
    fn buffered(&self) -> &[u8] {
        self.buffer.buffer()
    }
    /// TODO WIP
    fn flush_if_completed_line(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let this = self.project();
        //let this = &mut *self;
        match this.buffer.buffer().last().copied() {
            Some(b'\n') => this.buffer.flush_buf(cx),
            _ => Poll::Ready(Ok(())),
        }
    }
}

impl<W: AsyncWrite> AsyncWrite for LineWriterShim<W> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let mut this = self.as_mut().project();
        let newline_index = match memchr::memrchr(b'\n', buf) {
            None => {
                ready!(self.as_mut().flush_if_completed_line(cx)?);
                return self.project().buffer.poll_write(cx, buf);
            }
            Some(newline_index) => newline_index + 1,
        };

        this.buffer.as_mut().poll_flush(cx)?;

        let lines = &buf[..newline_index];

        let flushed = ready!(this.buffer.as_mut().poll_write(cx, lines))?;

        if flushed == 0 {
            return Poll::Ready(Ok(0));
        }

        let tail = if flushed >= newline_index {
            &buf[flushed..]
        } else if newline_index - flushed <= this.buffer.capacity() {
            &buf[flushed..newline_index]
        } else {
            let scan_area = &buf[flushed..];
            let scan_area = &scan_area[..this.buffer.capacity()];
            match memchr::memrchr(b'\n', scan_area) {
                Some(newline_index) => &scan_area[..newline_index + 1],
                None => scan_area,
            }
        };

        let buffered = this.buffer.write_to_buf(tail);
        Poll::Ready(Ok(flushed + buffered))
    }
    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.project().buffer.poll_flush(cx)
    }
    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.project().buffer.poll_close(cx)
    }
}
pin_project! {
/// TODO: WIP
#[derive(Debug)]
pub struct LineWriter<W: AsyncWrite> {
    #[pin]
    inner: BufWriter<W>,
}
}

impl<W: AsyncWrite> LineWriter<W> {
    /// TODO: WIP
    pub fn new(inner: W) -> LineWriter<W> {
        // 1024 is taken from std::io::buffered::LineWriter
        LineWriter::with_capacity(1024, inner)
    }
    /// TODO: WIP
    pub fn with_capacity(capacity: usize, inner: W) -> LineWriter<W> {
        LineWriter { inner: BufWriter::with_capacity(capacity, inner) }
    }
}

impl<W: AsyncWrite> AsyncWrite for LineWriter<W> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let lws = LineWriterShim { buffer: Box::new(&self.inner).as_mut() };
        pin_mut!(lws);
        lws.poll_write(cx, buf)

        //lws.poll_write(cx, buf)

        //Pin::new(&mut lws).poll_write(cx, buf)

        //Box::pin(&mut lws).poll_write(cx, buf)

        //Pin::new(&mut *lws).poll_write(cx, buf)
    }
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.project().inner.poll_flush(cx)
    }
    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.project().inner.poll_close(cx)
    }
}
