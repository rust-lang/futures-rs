//! Additional combinators for testing sinks.

use futures_sink::Sink;

pub use crate::assert_unmoved::AssertUnmoved;
pub use crate::interleave_pending::InterleavePending;
pub use crate::track_closed::TrackClosed;

/// Additional combinators for testing sinks.
pub trait SinkTestExt<Item>: Sink<Item> {
    /// Asserts that the given is not moved after being polled.
    ///
    /// A check for movement is performed each time the sink is polled
    /// and when `Drop` is called.
    ///
    /// Aside from keeping track of the location at which the sink was first
    /// polled and providing assertions, this sink adds no runtime behavior
    /// and simply delegates to the child sink.
    fn assert_unmoved_sink(self) -> AssertUnmoved<Self>
    where
        Self: Sized,
    {
        AssertUnmoved::new(self)
    }

    /// Introduces an extra [`Poll::Pending`](futures_core::task::Poll::Pending)
    /// in between each operation on the sink.
    fn interleave_pending_sink(self) -> InterleavePending<Self>
    where
        Self: Sized,
    {
        InterleavePending::new(self)
    }

    /// Track whether this sink has been closed and panics if it is used after closing.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures::executor::block_on(async {
    /// use futures::sink::{SinkExt, drain};
    /// use futures_test::sink::SinkTestExt;
    ///
    /// let mut sink = drain::<i32>().track_closed();
    ///
    /// sink.send(1).await?;
    /// assert!(!sink.is_closed());
    /// sink.close().await?;
    /// assert!(sink.is_closed());
    ///
    /// # Ok::<(), std::convert::Infallible>(()) })?;
    /// # Ok::<(), std::convert::Infallible>(())
    /// ```
    ///
    /// Note: Unlike [`AsyncWriteTestExt::track_closed`] when
    /// used as a sink the adaptor will panic if closed too early as there's no easy way to
    /// integrate as an error.
    ///
    /// [`AsyncWriteTestExt::track_closed`]: crate::io::AsyncWriteTestExt::track_closed
    ///
    /// ```
    /// # futures::executor::block_on(async {
    /// use std::panic::AssertUnwindSafe;
    /// use futures::{sink::{SinkExt, drain}, future::FutureExt};
    /// use futures_test::sink::SinkTestExt;
    ///
    /// let mut sink = drain::<i32>().track_closed();
    ///
    /// sink.close().await?;
    /// assert!(AssertUnwindSafe(sink.send(1)).catch_unwind().await.is_err());
    /// # Ok::<(), std::convert::Infallible>(()) })?;
    /// # Ok::<(), std::convert::Infallible>(())
    /// ```
    fn track_closed(self) -> TrackClosed<Self>
    where
        Self: Sized,
    {
        TrackClosed::new(self)
    }
}

impl<Item, W> SinkTestExt<Item> for W where W: Sink<Item> {}
