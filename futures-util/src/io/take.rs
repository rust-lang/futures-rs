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

    /// Returns the remaining number of bytes that can be
    /// read before this instance will return EOF.
    ///
    /// # Note
    ///
    /// This instance may reach `EOF` after reading fewer bytes than indicated by
    /// this method if the underlying [`AsyncRead`] instance reaches EOF.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(async_await)]
    /// # futures::executor::block_on(async {
    /// use futures::io::AsyncReadExt;
    /// use std::io::Cursor;
    ///
    /// let reader = Cursor::new(&b"12345678"[..]);
    /// let mut buffer = [0; 2];
    ///
    /// let mut take = reader.take(4);
    /// let n = take.read(&mut buffer).await?;
    ///
    /// assert_eq!(take.limit(), 2);
    /// # Ok::<(), Box<dyn std::error::Error>>(()) }).unwrap();
    /// ```
    pub fn limit(&self) -> u64 {
        self.limit
    }

    /// Sets the number of bytes that can be read before this instance will
    /// return EOF. This is the same as constructing a new `Take` instance, so
    /// the amount of bytes read and the previous limit value don't matter when
    /// calling this method.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(async_await)]
    /// # futures::executor::block_on(async {
    /// use futures::io::AsyncReadExt;
    /// use std::io::Cursor;
    ///
    /// let reader = Cursor::new(&b"12345678"[..]);
    /// let mut buffer = [0; 4];
    ///
    /// let mut take = reader.take(4);
    /// let n = take.read(&mut buffer).await?;
    ///
    /// assert_eq!(n, 4);
    /// assert_eq!(take.limit(), 0);
    ///
    /// take.set_limit(10);
    /// let n = take.read(&mut buffer).await?;
    /// assert_eq!(n, 4);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(()) }).unwrap();
    /// ```
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
