use futures_core::{Poll, Future};
use futures_core::task;
use futures_sink::Sink;

use core::mem::PinMut;

/// Future for the `close` combinator, which polls the sink until all data has
/// been closed.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct Close<'a, S: 'a + ?Sized> {
    sink: PinMut<'a, S>,
}

/// A future that completes when the sink has finished closing.
///
/// The sink itself is returned after closeing is complete.
pub fn new<'a, S: Sink + ?Sized>(sink: PinMut<'a, S>) -> Close<'a, S> {
    Close { sink }
}

impl<'a, S: Sink + ?Sized> Future for Close<'a, S> {
    type Output = Result<(), S::SinkError>;

    fn poll(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<Self::Output> {
        self.sink.reborrow().poll_close(cx)
    }
}
