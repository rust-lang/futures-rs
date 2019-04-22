use crate::stream::{StreamExt, Fuse};
use core::pin::Pin;
use futures_core::stream::{FusedStream, Stream};
use futures_core::task::{Context, Poll};

/// Stream for the [`select`](super::StreamExt::select) method.
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct Select<St1, St2> {
    stream1: Fuse<St1>,
    stream2: Fuse<St2>,
    flag: bool,
}

impl<St1: Unpin, St2: Unpin> Unpin for Select<St1, St2> {}

impl<St1, St2> Select<St1, St2>
    where St1: Stream,
          St2: Stream<Item = St1::Item>
{
    pub(super) fn new(stream1: St1, stream2: St2) -> Select<St1, St2> {
        Select {
            stream1: stream1.fuse(),
            stream2: stream2.fuse(),
            flag: false,
        }
    }

    /// Acquires a reference to the underlying streams that this combinator is
    /// pulling from.
    pub fn get_ref(&self) -> (&St1, &St2) {
        (self.stream1.get_ref(), self.stream2.get_ref())
    }

    /// Acquires a mutable reference to the underlying streams that this
    /// combinator is pulling from.
    ///
    /// Note that care must be taken to avoid tampering with the state of the
    /// stream which may otherwise confuse this combinator.
    pub fn get_mut(&mut self) -> (&mut St1, &mut St2) {
        (self.stream1.get_mut(), self.stream2.get_mut())
    }

    /// Acquires a pinned mutable reference to the underlying streams that this
    /// combinator is pulling from.
    ///
    /// Note that care must be taken to avoid tampering with the state of the
    /// stream which may otherwise confuse this combinator.
    pub fn get_pin_mut<'a>(self: Pin<&'a mut Self>) -> (Pin<&'a mut St1>, Pin<&'a mut St2>)
        where St1: Unpin, St2: Unpin,
    {
        let Self { stream1, stream2, .. } = Pin::get_mut(self);
        (Pin::new(stream1.get_mut()), Pin::new(stream2.get_mut()))
    }

    /// Consumes this combinator, returning the underlying streams.
    ///
    /// Note that this may discard intermediate state of this combinator, so
    /// care should be taken to avoid losing resources when this is called.
    pub fn into_inner(self) -> (St1, St2) {
        (self.stream1.into_inner(), self.stream2.into_inner())
    }
}

impl<St1, St2> FusedStream for Select<St1, St2> {
    fn is_terminated(&self) -> bool {
        self.stream1.is_terminated() && self.stream2.is_terminated()
    }
}

impl<St1, St2> Stream for Select<St1, St2>
    where St1: Stream,
          St2: Stream<Item = St1::Item>
{
    type Item = St1::Item;

    fn poll_next(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<St1::Item>> {
        let Select { flag, stream1, stream2 } =
            unsafe { Pin::get_unchecked_mut(self) };
        let stream1 = unsafe { Pin::new_unchecked(stream1) };
        let stream2 = unsafe { Pin::new_unchecked(stream2) };

        if !*flag {
            poll_inner(flag, stream1, stream2, cx)
        } else {
            poll_inner(flag, stream2, stream1, cx)
        }
    }
}

fn poll_inner<St1, St2>(
    flag: &mut bool,
    a: Pin<&mut St1>,
    b: Pin<&mut St2>,
    cx: &mut Context<'_>
) -> Poll<Option<St1::Item>>
    where St1: Stream, St2: Stream<Item = St1::Item>
{
    let a_done = match a.poll_next(cx) {
        Poll::Ready(Some(item)) => {
            // give the other stream a chance to go first next time
            *flag = !*flag;
            return Poll::Ready(Some(item))
        },
        Poll::Ready(None) => true,
        Poll::Pending => false,
    };

    match b.poll_next(cx) {
        Poll::Ready(Some(item)) => {
            Poll::Ready(Some(item))
        }
        Poll::Ready(None) if a_done => Poll::Ready(None),
        Poll::Ready(None) | Poll::Pending => Poll::Pending,
    }
}
