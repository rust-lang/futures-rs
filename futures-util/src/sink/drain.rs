use core::fmt;
use core::marker::PhantomData;
use core::pin::Pin;
use futures_core::task::{Context, Poll};
use futures_sink::Sink;

/// Sink for the [`drain`] function.
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
/// #![feature(async_await, await_macro)]
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

impl<T> Sink<T> for Drain<T> {
    type SinkError = DrainError;

    fn poll_ready(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<Result<(), Self::SinkError>> {
        Poll::Ready(Ok(()))
    }

    fn start_send(
        self: Pin<&mut Self>,
        _item: T,
    ) -> Result<(), Self::SinkError> {
        Ok(())
    }

    fn poll_flush(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<Result<(), Self::SinkError>> {
        Poll::Ready(Ok(()))
    }

    fn poll_close(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<Result<(), Self::SinkError>> {
        Poll::Ready(Ok(()))
    }
}

impl fmt::Display for DrainError {
    fn fmt(&self, _: &mut fmt::Formatter<'_>) -> fmt::Result {
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
