use futures_core::future::Future;
use futures_core::task::{Poll, Context};
use futures_sink::Sink;

use core::marker::Unpin;
use core::mem::PinMut;

/// Future for the `close` combinator, which polls the sink until all data has
/// been closed.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct Close<'a, S: 'a + Unpin + ?Sized> {
    sink: &'a mut S,
}

/// A future that completes when the sink has finished closing.
///
/// The sink itself is returned after closeing is complete.
pub fn new<'a, S: Sink + Unpin + ?Sized>(sink: &'a mut S) -> Close<'a, S> {
    Close { sink }
}

impl<'a, S: Sink + Unpin + ?Sized> Future for Close<'a, S> {
    type Output = Result<(), S::SinkError>;

    fn poll(mut self: PinMut<Self>, cx: &mut Context) -> Poll<Self::Output> {
        PinMut::new(&mut self.sink).poll_close(cx)
    }
}
