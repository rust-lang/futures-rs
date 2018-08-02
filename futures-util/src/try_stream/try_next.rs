use futures_core::future::Future;
use futures_core::stream::TryStream;
use futures_core::task::{self, Poll};
use core::marker::Unpin;
use core::mem::PinMut;

/// A future which attempts to collect all of the values of a stream.
///
/// This future is created by the `Stream::try_collect` method.
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct TryNext<'a, St: 'a> {
    stream: &'a mut St,
}

impl<'a, St: TryStream + Unpin> Unpin for TryNext<'a, St> {}

impl<'a, St: TryStream + Unpin> TryNext<'a, St> {
    pub(super) fn new(stream: &'a mut St) -> Self {
        TryNext { stream }
    }
}

impl<'a, St: TryStream + Unpin> Future for TryNext<'a, St> {
    type Output = Result<Option<St::Ok>, St::Error>;

    fn poll(
        mut self: PinMut<Self>,
        cx: &mut task::Context,
    ) -> Poll<Self::Output> {
        match PinMut::new(&mut *self.stream).try_poll_next(cx) {
            Poll::Ready(Some(Ok(x))) => Poll::Ready(Ok(Some(x))),
            Poll::Ready(Some(Err(e))) => Poll::Ready(Err(e)),
            Poll::Ready(None) => Poll::Ready(Ok(None)),
            Poll::Pending => Poll::Pending,
        }
    }
}
