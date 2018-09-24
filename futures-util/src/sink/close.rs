use core::marker::Unpin;
use core::pin::Pin;
use futures_core::future::Future;
use futures_core::task::{self, Poll};
use futures_sink::Sink;

/// Future for the `close` combinator, which polls the sink until all data has
/// been closed.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct Close<'a, Si: 'a + Unpin + ?Sized> {
    sink: &'a mut Si,
}

/// A future that completes when the sink has finished closing.
///
/// The sink itself is returned after closeing is complete.
impl<'a, Si: Sink + Unpin + ?Sized> Close<'a, Si> {
    pub(super) fn new(sink: &'a mut Si) -> Self {
        Close { sink }
    }
}

impl<Si: Sink + Unpin + ?Sized> Future for Close<'_, Si> {
    type Output = Result<(), Si::SinkError>;

    fn poll(
        mut self: Pin<&mut Self>,
        lw: &LocalWaker,
    ) -> Poll<Self::Output> {
        Pin::new(&mut self.sink).poll_close(cx)
    }
}
