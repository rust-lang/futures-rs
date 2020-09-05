//! Additional combinators for testing sinks.

use futures_sink::Sink;

pub use crate::track_closed::TrackClosed;

/// Additional combinators for testing sinks.
pub trait SinkTestExt<Item>: Sink<Item> {
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
