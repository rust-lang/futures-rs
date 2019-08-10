use futures_core::task::{Context, Poll};
use futures_io::AsyncRead;
use std::io;
use std::pin::Pin;

/// Future for the [`take`](super::AsyncReadExt::take) method.
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct Take<R: Unpin> {
    inner: R,
    limit: u64,
}

impl<R: Unpin> Unpin for Take<R> { }

impl<R: AsyncRead + Unpin> Take<R> {
    pub(super) fn new(inner: R, limit: u64) -> Self {
        Take { inner, limit }
    }

    pub fn limit(&self) -> u64 {
        self.limit
    }

    pub fn set_limit(&mut self, limit: u64) {
        self.limit = limit
    }
}

impl<R: AsyncRead + Unpin> AsyncRead for Take<R> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<Result<usize, io::Error>> {
        if self.limit == 0 {
            return Poll::Ready(Ok(0));
        }

        let max = std::cmp::min(buf.len() as u64, self.limit) as usize;
        let n = ready!(Pin::new(&mut self.inner).poll_read(cx, &mut buf[..max]))?;
        self.limit -= n as u64;
        Poll::Ready(Ok(n))
    }
}
