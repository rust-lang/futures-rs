use crate::stream::{StreamExt, TryStreamExt, Fuse};
use core::fmt;
use core::pin::Pin;
use futures_core::future::Future;
use futures_core::iteration;
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
    sink: &'a mut Si,
    stream: Fuse<&'a mut St>,
    buffered: Option<St::Ok>,
    yield_after: iteration::Limit,
}

impl<Si, St> fmt::Debug for SendAll<'_, Si, St>
where
    Si: fmt::Debug + ?Sized,
    St: fmt::Debug + ?Sized + TryStream,
    St::Ok: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SendAll")
            .field("sink", &self.sink)
            .field("stream", &self.stream)
            .field("buffered", &self.buffered)
            .field("yield_after", &self.yield_after)
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
    future_method_yield_after_every! {
        #[doc = "the underlying stream and the sink"]
        #[doc = "the stream consecutively yields items that the sink
            is ready to accept,"]
    }

    pub(super) fn new(
        sink: &'a mut Si,
        stream: &'a mut St,
    ) -> SendAll<'a, Si, St> {
        SendAll {
            sink,
            stream: stream.fuse(),
            buffered: None,
            yield_after: crate::DEFAULT_YIELD_AFTER_LIMIT,
        }
    }
}

fn try_start_send<Si, Ok>(
    mut sink: Pin<&mut Si>,
    cx: &mut Context<'_>,
    item: Ok,
    buffered: &mut Option<Ok>,
) -> Poll<Result<(), Si::Error>>
where
    Si: Sink<Ok> + ?Sized,
{
    debug_assert!(buffered.is_none());
    match sink.as_mut().poll_ready(cx)? {
        Poll::Ready(()) => Poll::Ready(sink.as_mut().start_send(item)),
        Poll::Pending => {
            *buffered = Some(item);
            Poll::Pending
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
        let (stream, mut sink, buffered) = (
            &mut this.stream,
            Pin::new(&mut this.sink),
            &mut this.buffered,
        );

        // If we've got an item buffered already, we need to write it to the
        // sink before we can do anything else
        if let Some(item) = buffered.take() {
            ready!(try_start_send(sink.as_mut(), cx, item, buffered))?
        }

        poll_loop! { &mut this.yield_after, cx,
            match stream.try_poll_next_unpin(cx)? {
                Poll::Ready(Some(item)) => {
                    ready!(try_start_send(sink.as_mut(), cx, item, buffered))?
                }
                Poll::Ready(None) => {
                    ready!(sink.as_mut().poll_flush(cx))?;
                    return Poll::Ready(Ok(()))
                }
                Poll::Pending => {
                    ready!(sink.as_mut().poll_flush(cx))?;
                    return Poll::Pending
                }
            }
        }
    }
}
