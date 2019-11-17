use crate::stream::{StreamExt, Fuse};
use core::pin::Pin;
use futures_core::future::{FusedFuture, Future};
use futures_core::iteration;
use futures_core::stream::{Stream, TryStream};
use futures_core::task::{Context, Poll};
use futures_sink::Sink;
use pin_utils::{unsafe_pinned, unsafe_unpinned};

const INVALID_POLL: &str = "polled `Forward` after completion";

/// Future for the [`forward`](super::StreamExt::forward) method.
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct Forward<St: TryStream, Si> {
    sink: Option<Si>,
    stream: Fuse<St>,
    buffered_item: Option<St::Ok>,
    yield_after: iteration::Limit,
}

struct Borrows<'a, St: TryStream, Si> {
    sink: Pin<&'a mut Option<Si>>,
    stream: Pin<&'a mut Fuse<St>>,
    buffered_item: &'a mut Option<St::Ok>,
    yield_after: &'a mut iteration::Limit,
}

impl<St: TryStream + Unpin, Si: Unpin> Unpin for Forward<St, Si> {}

impl<St, Si, E> Forward<St, Si>
where
    Si: Sink<St::Ok, Error = E>,
    St: TryStream<Error = E> + Stream,
{
    unsafe_pinned!(sink: Option<Si>);
    unsafe_pinned!(stream: Fuse<St>);
    unsafe_unpinned!(buffered_item: Option<St::Ok>);
    unsafe_unpinned!(yield_after: iteration::Limit);

    fn split_borrows(self: Pin<&mut Self>) -> Borrows<'_, St, Si> {
        unsafe {
            let this = self.get_unchecked_mut();
            Borrows {
                sink: Pin::new_unchecked(&mut this.sink),
                stream: Pin::new_unchecked(&mut this.stream),
                buffered_item: &mut this.buffered_item,
                yield_after: &mut this.yield_after,
            }
        }
    }

    future_method_yield_after_every! {
        #[doc = "the underlying stream and the sink"]
        #[doc = "the stream consecutively yields items that the sink
            is ready to accept,"]
    }

    pub(super) fn new(stream: St, sink: Si) -> Self {
        Forward {
            sink: Some(sink),
            stream: stream.fuse(),
            buffered_item: None,
            yield_after: crate::DEFAULT_YIELD_AFTER_LIMIT,
        }
    }
}

fn try_start_send<Si, Ok>(
    sink: Pin<&mut Option<Si>>,
    cx: &mut Context<'_>,
    item: Ok,
    buffered_item: &mut Option<Ok>,
) -> Poll<Result<(), Si::Error>>
where
    Si: Sink<Ok>,
{
    debug_assert!(buffered_item.is_none());
    {
        let mut sink = sink.as_pin_mut().unwrap();
        if sink.as_mut().poll_ready(cx)?.is_ready() {
            return Poll::Ready(sink.start_send(item));
        }
    }
    *buffered_item = Some(item);
    Poll::Pending
}

impl<St, Si, Item, E> FusedFuture for Forward<St, Si>
where
    Si: Sink<Item, Error = E>,
    St: Stream<Item = Result<Item, E>>,
{
    fn is_terminated(&self) -> bool {
        self.sink.is_none()
    }
}

impl<St, Si, Item, E> Future for Forward<St, Si>
where
    Si: Sink<Item, Error = E>,
    St: Stream<Item = Result<Item, E>>,
{
    type Output = Result<(), E>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let Borrows {
            mut sink,
            mut stream,
            buffered_item,
            yield_after,
        } = self.split_borrows();

        // If we've got an item buffered already, we need to write it to the
        // sink before we can do anything else
        if let Some(item) = buffered_item.take() {
            ready!(try_start_send(sink.as_mut(), cx, item, buffered_item))?;
        }

        poll_loop! { yield_after, cx,
            match stream.as_mut().poll_next(cx)? {
                Poll::Ready(Some(item)) =>
                    ready!(try_start_send(sink.as_mut(), cx, item, buffered_item))?,
                Poll::Ready(None) => {
                    ready!(sink.as_mut().as_pin_mut().expect(INVALID_POLL).poll_close(cx))?;
                    sink.as_mut().set(None);
                    return Poll::Ready(Ok(()))
                }
                Poll::Pending => {
                    ready!(sink.as_mut().as_pin_mut().expect(INVALID_POLL).poll_flush(cx))?;
                    return Poll::Pending
                }
            }
        }
    }
}
