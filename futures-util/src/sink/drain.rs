use core::fmt;
use core::marker::PhantomData;
use core::pin::Pin;
use futures_core::task::{self, Poll};
use futures_sink::Sink;

/// A sink that will discard all items given to it.
///
/// See the [`drain()`] function for more details.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct Drain<T> {
    marker: PhantomData<T>,
}

/// The error type for the [`Drain`] sink.
#[derive(Debug)]
pub enum DrainError {
}

/// Create a sink that will just discard all items given to it.
///
/// Similar to [`io::Sink`](::std::io::Sink).
///
/// # Examples
///
/// ```
/// #![feature(async_await, await_macro, futures_api)]
/// # futures::executor::block_on(async {
/// use futures::sink::{self, SinkExt};
///
/// let mut drain = sink::drain();
/// await!(drain.send(5))?;
/// # Ok::<(), futures::sink::DrainError>(()) }).unwrap();
/// ```
pub fn drain<T>() -> Drain<T> {
    Drain { marker: PhantomData }
}

impl<T> Sink for Drain<T> {
    type SinkItem = T;
    type SinkError = DrainError;

    fn poll_ready(
        self: Pin<&mut Self>,
        _lw: &LocalWaker,
    ) -> Poll<Result<(), Self::SinkError>> {
        Poll::Ready(Ok(()))
    }

    fn start_send(
        self: Pin<&mut Self>,
        _item: Self::SinkItem,
    ) -> Result<(), Self::SinkError> {
        Ok(())
    }

    fn poll_flush(
        self: Pin<&mut Self>,
        _lw: &LocalWaker,
    ) -> Poll<Result<(), Self::SinkError>> {
        Poll::Ready(Ok(()))
    }

    fn poll_close(
        self: Pin<&mut Self>,
        _lw: &LocalWaker,
    ) -> Poll<Result<(), Self::SinkError>> {
        Poll::Ready(Ok(()))
    }
}

impl fmt::Display for DrainError {
    fn fmt(&self, _: &mut fmt::Formatter) -> fmt::Result {
        match *self {
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for DrainError {
}

impl DrainError {
    /// Convert this drain error into any type
    pub fn into_any<T>(self) -> T {
        match self {
        }
    }
}
