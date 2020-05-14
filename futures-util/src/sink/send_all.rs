use super::FeedAll;
use core::fmt;
use core::pin::Pin;
use futures_core::future::Future;
use futures_core::ready;
use futures_core::stream::{TryStream, Stream};
use futures_core::task::{Context, Poll};
use futures_sink::Sink;

/// Future for the [`send_all`](super::SinkExt::send_all) method.
#[allow(explicit_outlives_requirements)] // https://github.com/rust-lang/rust/issues/60993
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct SendAll<'a, Si, St>
where
    Si: ?Sized,
    St: ?Sized + TryStream,
{
    feed_all: FeedAll<'a, Si, St>,
    is_flushing: bool,
}

impl<Si, St> fmt::Debug for SendAll<'_, Si, St>
where
    Si: fmt::Debug + ?Sized,
    St: fmt::Debug + ?Sized + TryStream,
    St::Ok: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SendAll")
            .field("feed_all", &self.feed_all)
            .field("is_flushing", &self.is_flushing)
            .finish()
    }
}

// Pinning is never projected to any fields
impl<Si, St> Unpin for SendAll<'_, Si, St>
where
    Si: Unpin + ?Sized,
    St: TryStream + Unpin + ?Sized,
{}

impl<'a, Si, St, Ok, Error> SendAll<'a, Si, St>
where
    Si: Sink<Ok, Error = Error> + Unpin + ?Sized,
    St: TryStream<Ok = Ok, Error = Error> + Stream + Unpin + ?Sized,
{
    pub(super) fn new(
        sink: &'a mut Si,
        stream: &'a mut St,
    ) -> Self {
        Self {
            feed_all: FeedAll::new(sink, stream),
            is_flushing: false,
        }
    }
}

impl<Si, St, Ok, Error> Future for SendAll<'_, Si, St>
where
    Si: Sink<Ok, Error = Error> + Unpin + ?Sized,
    St: Stream<Item = Result<Ok, Error>> + Unpin + ?Sized,
{
    type Output = Result<(), Error>;

    fn poll(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Self::Output> {
        let this = &mut *self;

        if !this.is_flushing {
            ready!(Pin::new(&mut this.feed_all).poll(cx))?;
            this.is_flushing = true;
        }

        ready!(this.feed_all.sink_pin_mut().poll_flush(cx))?;
        Poll::Ready(Ok(()))
    }
}
