use core::marker::Unpin;
use core::mem::PinMut;
use futures_core::future::Future;
use futures_core::task::{self, Poll};
use futures_sink::Sink;

/// Future for the `flush` combinator, which polls the sink until all data
/// has been flushed.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct Flush<'a, Si: 'a + Unpin + ?Sized> {
    sink: &'a mut Si,
}

// Pin is never projected to a field.
impl<'a, Si: Unpin + ?Sized> Unpin for Flush<'a, Si> {}

/// A future that completes when the sink has finished processing all
/// pending requests.
///
/// The sink itself is returned after flushing is complete; this adapter is
/// intended to be used when you want to stop sending to the sink until
/// all current requests are processed.
impl<'a, Si: Sink + Unpin + ?Sized> Flush<'a, Si> {
    pub(super) fn new(sink: &'a mut Si) -> Flush<'a, Si> {
        Flush { sink }
    }
}

impl<'a, Si: Sink + Unpin + ?Sized> Future for Flush<'a, Si> {
    type Output = Result<(), Si::SinkError>;

    fn poll(
        mut self: PinMut<Self>,
        cx: &mut task::Context,
    ) -> Poll<Self::Output> {
        PinMut::new(&mut self.sink).poll_flush(cx)
    }
}
