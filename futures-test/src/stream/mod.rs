//! Additional combinators for testing streams.

use futures_core::stream::Stream;

pub use crate::interleave_pending::InterleavePending;

/// Additional combinators for testing streams.
pub trait StreamTestExt: Stream {
    /// Introduces an extra [`Poll::Pending`](futures_core::task::Poll::Pending)
    /// in between each item of the stream.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures::task::Poll;
    /// use futures::stream::{self, Stream};
    /// use futures_test::task::noop_context;
    /// use futures_test::stream::StreamTestExt;
    /// use futures::pin_mut;
    ///
    /// let stream = stream::iter(vec![1, 2]).interleave_pending();
    /// pin_mut!(stream);
    ///
    /// let mut cx = noop_context();
    ///
    /// assert_eq!(stream.as_mut().poll_next(&mut cx), Poll::Pending);
    /// assert_eq!(stream.as_mut().poll_next(&mut cx), Poll::Ready(Some(1)));
    /// assert_eq!(stream.as_mut().poll_next(&mut cx), Poll::Pending);
    /// assert_eq!(stream.as_mut().poll_next(&mut cx), Poll::Ready(Some(2)));
    /// assert_eq!(stream.as_mut().poll_next(&mut cx), Poll::Pending);
    /// assert_eq!(stream.as_mut().poll_next(&mut cx), Poll::Ready(None));
    /// ```
    fn interleave_pending(self) -> InterleavePending<Self>
    where
        Self: Sized,
    {
        InterleavePending::new(self)
    }
}

impl<St> StreamTestExt for St where St: Stream {}
