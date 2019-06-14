//! Additional combinators for testing async readers.

use futures_io::AsyncRead;

pub use super::interleave_pending::InterleavePending;

/// Additional combinators for testing async readers.
pub trait AsyncReadTestExt: AsyncRead {
    /// Introduces an extra [`Poll::Pending`](futures_core::task::Poll::Pending)
    /// in between each read of the reader.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(async_await)]
    /// use futures::task::Poll;
    /// use futures::io::AsyncRead;
    /// use futures_test::task::noop_context;
    /// use futures_test::io::AsyncReadTestExt;
    /// use pin_utils::pin_mut;
    ///
    /// let reader = std::io::Cursor::new(&[1, 2, 3]).interleave_pending();
    /// pin_mut!(reader);
    ///
    /// let mut cx = noop_context();
    ///
    /// let mut buf = [0, 0];
    ///
    /// assert_eq!(reader.as_mut().poll_read(&mut cx, &mut buf[..])?, Poll::Pending);
    /// assert_eq!(reader.as_mut().poll_read(&mut cx, &mut buf[..])?, Poll::Ready(2));
    /// assert_eq!(buf, [1, 2]);
    /// assert_eq!(reader.as_mut().poll_read(&mut cx, &mut buf[..])?, Poll::Pending);
    /// assert_eq!(reader.as_mut().poll_read(&mut cx, &mut buf[..])?, Poll::Ready(1));
    /// assert_eq!(buf, [3, 2]);
    /// assert_eq!(reader.as_mut().poll_read(&mut cx, &mut buf[..])?, Poll::Pending);
    /// assert_eq!(reader.as_mut().poll_read(&mut cx, &mut buf[..])?, Poll::Ready(0));
    ///
    /// # Ok::<(), std::io::Error>(())
    /// ```
    ///
    /// ## `AsyncBufRead`
    ///
    /// The returned reader will also implement `AsyncBufRead` if the underlying reader does.
    ///
    /// ```
    /// #![feature(async_await)]
    /// use futures::task::Poll;
    /// use futures::io::AsyncBufRead;
    /// use futures_test::task::noop_context;
    /// use futures_test::io::AsyncReadTestExt;
    /// use pin_utils::pin_mut;
    ///
    /// let reader = std::io::Cursor::new(&[1, 2, 3]).interleave_pending();
    /// pin_mut!(reader);
    ///
    /// let mut cx = noop_context();
    ///
    /// assert_eq!(reader.as_mut().poll_fill_buf(&mut cx)?, Poll::Pending);
    /// assert_eq!(reader.as_mut().poll_fill_buf(&mut cx)?, Poll::Ready(&[1, 2, 3][..]));
    /// reader.as_mut().consume(2);
    /// assert_eq!(reader.as_mut().poll_fill_buf(&mut cx)?, Poll::Pending);
    /// assert_eq!(reader.as_mut().poll_fill_buf(&mut cx)?, Poll::Ready(&[3][..]));
    /// reader.as_mut().consume(1);
    /// assert_eq!(reader.as_mut().poll_fill_buf(&mut cx)?, Poll::Pending);
    /// assert_eq!(reader.as_mut().poll_fill_buf(&mut cx)?, Poll::Ready(&[][..]));
    ///
    /// # Ok::<(), std::io::Error>(())
    /// ```
    fn interleave_pending(self) -> InterleavePending<Self>
    where
        Self: Sized,
    {
        InterleavePending::new(self)
    }
}

impl<R> AsyncReadTestExt for R where R: AsyncRead {}
