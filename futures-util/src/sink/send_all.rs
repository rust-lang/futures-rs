use crate::stream::{StreamExt, TryStreamExt, Fuse, IntoStream};
use core::fmt;
use core::pin::Pin;
use futures_core::future::Future;
use futures_core::stream::TryStream;
use futures_core::task::{Context, Poll};
use futures_sink::Sink;

/// Future for the [`send_all`](super::SinkExt::send_all) method.
#[allow(explicit_outlives_requirements)] // https://github.com/rust-lang/rust/issues/60993
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct SendAll<'a, Si, St>
where
    Si: ?Sized,
    St: ?Sized,
    &'a mut St: TryStream,
{
    sink: &'a mut Si,
    stream: Fuse<IntoStream<&'a mut St>>,
    buffered: Option<<&'a mut St as TryStream>::Ok>,
}

impl<'a, Si, St> fmt::Debug for SendAll<'a, Si, St>
where
    Si: fmt::Debug + ?Sized,
    St: fmt::Debug + ?Sized,
    &'a mut St: TryStream,
    <&'a mut St as TryStream>::Ok: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SendAll")
            .field("sink", &self.sink)
            .field("stream", &self.stream)
            .field("buffered", &self.buffered)
            .finish()
    }
}

// Pinning is never projected to any fields
impl<'a, Si, St> Unpin for SendAll<'a, Si, St>
where
    Si: Unpin + ?Sized,
    St: ?Sized,
    &'a mut St: TryStream + Unpin,
{}

impl<'a, Si, St, Ok, Error> SendAll<'a, Si, St>
where
    Si: Sink<Ok, Error = Error> + Unpin + ?Sized,
    St: ?Sized,
    &'a mut St: TryStream<Ok = Ok, Error = Error> + Unpin,
{
    pub(super) fn new(
        sink: &'a mut Si,
        stream: &'a mut St,
    ) -> SendAll<'a, Si, St> {
        SendAll {
            sink,
            stream: stream.into_stream().fuse(),
            buffered: None,
        }
    }

    fn try_start_send(
        &mut self,
        cx: &mut Context<'_>,
        item: <&'a mut St as TryStream>::Ok,
    ) -> Poll<Result<(), Si::Error>> {
        debug_assert!(self.buffered.is_none());
        match Pin::new(&mut self.sink).poll_ready(cx)? {
            Poll::Ready(()) => {
                Poll::Ready(Pin::new(&mut self.sink).start_send(item))
            }
            Poll::Pending => {
                self.buffered = Some(item);
                Poll::Pending
            }
        }
    }
}

impl<'a, Si, St, Ok, Error> Future for SendAll<'a, Si, St>
where
    Si: Sink<Ok, Error = Error> + Unpin + ?Sized,
    St: ?Sized,
    &'a mut St: TryStream<Ok = Ok, Error = Error> + Unpin,
{
    type Output = Result<(), Error>;

    fn poll(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Self::Output> {
        let this = &mut *self;
        // If we've got an item buffered already, we need to write it to the
        // sink before we can do anything else
        if let Some(item) = this.buffered.take() {
            ready!(this.try_start_send(cx, item))?
        }

        loop {
            match this.stream.try_poll_next_unpin(cx)? {
                Poll::Ready(Some(item)) => {
                    ready!(this.try_start_send(cx, item))?
                }
                Poll::Ready(None) => {
                    ready!(Pin::new(&mut this.sink).poll_flush(cx))?;
                    return Poll::Ready(Ok(()))
                }
                Poll::Pending => {
                    ready!(Pin::new(&mut this.sink).poll_flush(cx))?;
                    return Poll::Pending
                }
            }
        }
    }
}
