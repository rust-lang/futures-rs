use futures_core::{Poll, Future};
use futures_core::task;
use futures_sink::Sink;

use core::marker::Unpin;
use core::mem::PinMut;

/// Future for the `flush` combinator, which polls the sink until all data
/// has been flushed.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct Flush<'a, S: 'a + Unpin + ?Sized> {
    sink: &'a mut S,
}

// Pin is never projected to a field.
impl<'a, S: Unpin + ?Sized> Unpin for Flush<'a, S> {}

/// A future that completes when the sink has finished processing all
/// pending requests.
///
/// The sink itself is returned after flushing is complete; this adapter is
/// intended to be used when you want to stop sending to the sink until
/// all current requests are processed.
pub fn new<'a, S: Sink + Unpin + ?Sized>(sink: &'a mut S) -> Flush<'a, S> {
    Flush { sink }
}

impl<'a, S: Sink + Unpin + ?Sized> Future for Flush<'a, S> {
    type Output = Result<(), S::SinkError>;

    fn poll(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<Self::Output> {
        PinMut::new(&mut self.sink).poll_flush(cx)
    }
}
