use crate::stream::{Fuse, StreamExt};
use core::fmt;
use core::pin::Pin;
use futures_core::future::Future;
use futures_core::ready;
use futures_core::stream::{Stream, TryStream};
use futures_core::task::{Context, Poll};
use futures_sink::Sink;
use pin_project_lite::pin_project;

pin_project! {
    /// Future for the [`send_all`](super::SinkExt::send_all) method.
    #[allow(explicit_outlives_requirements)] // https://github.com/rust-lang/rust/issues/60993
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    pub struct SendAll<'a, Si, St>
    where
        Si: ?Sized,
        St: TryStream,
    {
        sink: &'a mut Si,
        #[pin]
        stream: Fuse<St>,
        buffered: Option<St::Ok>,
    }
}

impl<Si, St> fmt::Debug for SendAll<'_, Si, St>
where
    Si: fmt::Debug + ?Sized,
    St: fmt::Debug + TryStream,
    St::Ok: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SendAll")
            .field("sink", &self.sink)
            .field("stream", &self.stream)
            .field("buffered", &self.buffered)
            .finish()
    }
}

impl<'a, Si, St, Ok, Error> SendAll<'a, Si, St>
where
    Si: Sink<Ok, Error = Error> + Unpin + ?Sized,
    St: TryStream<Ok = Ok, Error = Error> + Stream,
{
    pub(super) fn new(sink: &'a mut Si, stream: St) -> Self {
        Self { sink, stream: stream.fuse(), buffered: None }
    }

    fn try_start_send(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        item: St::Ok,
    ) -> Poll<Result<(), Si::Error>> {
        let this = self.project();
        debug_assert!(this.buffered.is_none());
        match Pin::new(&mut *this.sink).poll_ready(cx)? {
            Poll::Ready(()) => Poll::Ready(Pin::new(&mut *this.sink).start_send(item)),
            Poll::Pending => {
                *this.buffered = Some(item);
                Poll::Pending
            }
        }
    }
}

impl<Si, St, Ok, Error> Future for SendAll<'_, Si, St>
where
    Si: Sink<Ok, Error = Error> + Unpin + ?Sized,
    St: Stream<Item = Result<Ok, Error>>,
{
    type Output = Result<(), Error>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // If we've got an item buffered already, we need to write it to the
        // sink before we can do anything else
        if let Some(item) = self.as_mut().project().buffered.take() {
            ready!(self.as_mut().try_start_send(cx, item))?
        }

        loop {
            let this = self.as_mut().project();
            match this.stream.try_poll_next(cx)? {
                Poll::Ready(Some(item)) => ready!(self.as_mut().try_start_send(cx, item))?,
                Poll::Ready(None) => {
                    ready!(Pin::new(this.sink).poll_flush(cx))?;
                    return Poll::Ready(Ok(()));
                }
                Poll::Pending => {
                    ready!(Pin::new(this.sink).poll_flush(cx))?;
                    return Poll::Pending;
                }
            }
        }
    }
}
