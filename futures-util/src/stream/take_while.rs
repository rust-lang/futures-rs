use core::fmt;
use core::pin::Pin;
use futures_core::future::Future;
use futures_core::stream::Stream;
use futures_core::task::{Context, Poll};
#[cfg(feature = "sink")]
use futures_sink::Sink;
use pin_project::{pin_project, unsafe_project};

/// Stream for the [`take_while`](super::StreamExt::take_while) method.
#[unsafe_project(Unpin)]
#[must_use = "streams do nothing unless polled"]
pub struct TakeWhile<St: Stream , Fut, F> {
    #[pin]
    stream: St,
    f: F,
    #[pin]
    pending_fut: Option<Fut>,
    pending_item: Option<St::Item>,
    done_taking: bool,
}

impl<St, Fut, F> fmt::Debug for TakeWhile<St, Fut, F>
where
    St: Stream + fmt::Debug,
    St::Item: fmt::Debug,
    Fut: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TakeWhile")
            .field("stream", &self.stream)
            .field("pending_fut", &self.pending_fut)
            .field("pending_item", &self.pending_item)
            .field("done_taking", &self.done_taking)
            .finish()
    }
}

impl<St, Fut, F> TakeWhile<St, Fut, F>
    where St: Stream,
          F: FnMut(&St::Item) -> Fut,
          Fut: Future<Output = bool>,
{
    pub(super) fn new(stream: St, f: F) -> TakeWhile<St, Fut, F> {
        TakeWhile {
            stream,
            f,
            pending_fut: None,
            pending_item: None,
            done_taking: false,
        }
    }

    /// Acquires a reference to the underlying stream that this combinator is
    /// pulling from.
    pub fn get_ref(&self) -> &St {
        &self.stream
    }

    /// Acquires a mutable reference to the underlying stream that this
    /// combinator is pulling from.
    ///
    /// Note that care must be taken to avoid tampering with the state of the
    /// stream which may otherwise confuse this combinator.
    pub fn get_mut(&mut self) -> &mut St {
        &mut self.stream
    }

    /// Acquires a pinned mutable reference to the underlying stream that this
    /// combinator is pulling from.
    ///
    /// Note that care must be taken to avoid tampering with the state of the
    /// stream which may otherwise confuse this combinator.
    #[pin_project(self)]
    pub fn get_pin_mut<'a>(self: Pin<&'a mut Self>) -> Pin<&'a mut St> {
        self.stream
    }

    /// Consumes this combinator, returning the underlying stream.
    ///
    /// Note that this may discard intermediate state of this combinator, so
    /// care should be taken to avoid losing resources when this is called.
    pub fn into_inner(self) -> St {
        self.stream
    }
}

impl<St, Fut, F> Stream for TakeWhile<St, Fut, F>
    where St: Stream,
          F: FnMut(&St::Item) -> Fut,
          Fut: Future<Output = bool>,
{
    type Item = St::Item;

    #[pin_project(self)]
    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<St::Item>> {
        if *self.done_taking {
            return Poll::Ready(None);
        }

        if self.pending_item.is_none() {
            let item = match ready!(self.stream.as_mut().poll_next(cx)) {
                Some(e) => e,
                None => return Poll::Ready(None),
            };
            let fut = (self.f)(&item);
            self.pending_fut.set(Some(fut));
            *self.pending_item = Some(item);
        }

        let take = ready!(self.pending_fut.as_mut().as_pin_mut().unwrap().poll(cx));
        self.pending_fut.set(None);
        let item = self.pending_item.take().unwrap();

        if take {
            Poll::Ready(Some(item))
        } else {
            *self.done_taking = true;
            Poll::Ready(None)
        }
    }
}

// Forwarding impl of Sink from the underlying stream
#[cfg(feature = "sink")]
impl<S, Fut, F, Item> Sink<Item> for TakeWhile<S, Fut, F>
    where S: Stream + Sink<Item>,
{
    type Error = S::Error;

    delegate_sink!(stream, Item);
}
