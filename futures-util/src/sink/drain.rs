use core::marker::PhantomData;
use core::mem::PinMut;
use futures_core::task::{self, Poll};
use futures_sink::Sink;

/// A sink that will discard all items given to it.
///
/// See the [`drain`] function for more details.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct Drain<T> {
    marker: PhantomData<T>,
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
/// # Ok::<(), ()>(()) }).unwrap();
/// ```
pub fn drain<T>() -> Drain<T> {
    Drain { marker: PhantomData }
}

impl<T> Sink for Drain<T> {
    type SinkItem = T;
    type SinkError = ();

    fn poll_ready(
        self: PinMut<Self>,
        _cx: &mut task::Context,
    ) -> Poll<Result<(), Self::SinkError>> {
        Poll::Ready(Ok(()))
    }

    fn start_send(
        self: PinMut<Self>,
        _item: Self::SinkItem,
    ) -> Result<(), Self::SinkError> {
        Ok(())
    }

    fn poll_flush(
        self: PinMut<Self>,
        _cx: &mut task::Context,
    ) -> Poll<Result<(), Self::SinkError>> {
        Poll::Ready(Ok(()))
    }

    fn poll_close(
        self: PinMut<Self>,
        _cx: &mut task::Context,
    ) -> Poll<Result<(), Self::SinkError>> {
        Poll::Ready(Ok(()))
    }
}
