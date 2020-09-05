//! Additional combinators for testing async readers.

use futures_io::AsyncRead;

pub use super::limited::Limited;
pub use crate::assert_unmoved::AssertUnmoved;
pub use crate::interleave_pending::InterleavePending;

/// Additional combinators for testing async readers.
pub trait AsyncReadTestExt: AsyncRead {
    /// Asserts that the given is not moved after being polled.
    ///
    /// A check for movement is performed each time the reader is polled
    /// and when `Drop` is called.
    ///
    /// Aside from keeping track of the location at which the reader was first
    /// polled and providing assertions, this reader adds no runtime behavior
    /// and simply delegates to the child reader.
    fn assert_unmoved(self) -> AssertUnmoved<Self>
    where
        Self: Sized,
    {
        AssertUnmoved::new(self)
    }

    /// Introduces an extra [`Poll::Pending`](futures_core::task::Poll::Pending)
    /// in between each read of the reader.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures::task::Poll;
    /// use futures::io::{AsyncRead, Cursor};
    /// use futures_test::task::noop_context;
    /// use futures_test::io::AsyncReadTestExt;
    /// use futures::pin_mut;
    ///
    /// let reader = Cursor::new(&[1, 2, 3]).interleave_pending();
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
    /// use futures::task::Poll;
    /// use futures::io::{AsyncBufRead, Cursor};
    /// use futures_test::task::noop_context;
    /// use futures_test::io::AsyncReadTestExt;
    /// use futures::pin_mut;
    ///
    /// let reader = Cursor::new(&[1, 2, 3]).interleave_pending();
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

    /// Limit the number of bytes allowed to be read on each call to `poll_read`.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures::task::Poll;
    /// use futures::io::{AsyncRead, Cursor};
    /// use futures_test::task::noop_context;
    /// use futures_test::io::AsyncReadTestExt;
    /// use futures::pin_mut;
    ///
    /// let reader = Cursor::new(&[1, 2, 3, 4, 5]).limited(2);
    /// pin_mut!(reader);
    ///
    /// let mut cx = noop_context();
    ///
    /// let mut buf = [0; 10];
    ///
    /// assert_eq!(reader.as_mut().poll_read(&mut cx, &mut buf)?, Poll::Ready(2));
    /// assert_eq!(&buf[..2], &[1, 2]);
    /// assert_eq!(reader.as_mut().poll_read(&mut cx, &mut buf)?, Poll::Ready(2));
    /// assert_eq!(&buf[..2], &[3, 4]);
    /// assert_eq!(reader.as_mut().poll_read(&mut cx, &mut buf)?, Poll::Ready(1));
    /// assert_eq!(&buf[..1], &[5]);
    ///
    /// # Ok::<(), std::io::Error>(())
    /// ```
    fn limited(self, limit: usize) -> Limited<Self>
    where
        Self: Sized,
    {
        Limited::new(self, limit)
    }
}

impl<R> AsyncReadTestExt for R where R: AsyncRead {}
