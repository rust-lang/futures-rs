use futures_core::future::Future;
use futures_core::task::{Context, Poll};
use futures_io::{AsyncBufRead, AsyncWrite};
use std::io;
use std::pin::Pin;

/// Future for the [`copy_buf_into`](super::AsyncBufReadExt::copy_buf_into) method.
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct CopyBufInto<'a, R, W: ?Sized> {
    reader: R,
    writer: &'a mut W,
    amt: u64,
}

impl<R: Unpin, W: ?Sized> Unpin for CopyBufInto<'_, R, W> {}

impl<R, W: ?Sized> CopyBufInto<'_, R, W> {
    pub(super) fn new(reader: R, writer: &mut W) -> CopyBufInto<'_, R, W> {
        CopyBufInto {
            reader,
            writer,
            amt: 0,
        }
    }
}

impl<R, W: Unpin + ?Sized> CopyBufInto<'_, R, W> {
    fn project<'b>(self: Pin<&'b mut Self>) -> (Pin<&'b mut R>, Pin<&'b mut W>, &'b mut u64) {
        unsafe {
            let this = self.get_unchecked_mut();
            (Pin::new_unchecked(&mut this.reader), Pin::new(&mut *this.writer), &mut this.amt)
        }
    }
}

impl<R, W> Future for CopyBufInto<'_, R, W>
    where R: AsyncBufRead,
          W: AsyncWrite + Unpin + ?Sized,
{
    type Output = io::Result<u64>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let (mut reader, mut writer, amt) = self.project();
        loop {
            let buffer = ready!(reader.as_mut().poll_fill_buf(cx))?;
            if buffer.is_empty() {
                ready!(writer.as_mut().poll_flush(cx))?;
                return Poll::Ready(Ok(*amt));
            }

            let i = ready!(writer.as_mut().poll_write(cx, buffer))?;
            if i == 0 {
                return Poll::Ready(Err(io::ErrorKind::WriteZero.into()))
            }
            *amt += i as u64;
            reader.as_mut().consume(i);
        }
    }
}
