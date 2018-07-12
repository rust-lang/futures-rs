use core::marker::Unpin;
use core::mem::PinMut;
use futures_core::future::Future;
use futures_core::stream::Stream;
use futures_core::task::{self, Poll};

/// A future of the next element of a stream.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct Next<'a, St: 'a> {
    stream: &'a mut St,
}

impl<'a, St: Stream + Unpin> Next<'a, St> {
    pub(super) fn new(stream: &'a mut St) -> Next<'a, St> {
        Next { stream }
    }
}

impl<'a, St: Stream + Unpin> Future for Next<'a, St> {
    type Output = Option<St::Item>;

    fn poll(
        mut self: PinMut<Self>,
        cx: &mut task::Context,
    ) -> Poll<Self::Output> {
        PinMut::new(&mut *self.stream).poll_next(cx)
    }
}
