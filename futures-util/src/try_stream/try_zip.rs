use crate::stream::{StreamExt, Fuse};
use crate::try_stream::{TryStreamExt, IntoStream};
use core::pin::Pin;
use futures_core::stream::{FusedStream, TryStream};
use futures_core::task::{Context, Poll};
use pin_utils::{unsafe_pinned, unsafe_unpinned};

/// Stream for the [`try_zip`](super::TryStreamExt::try_zip) method.
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct TryZip<St1: TryStream, St2: TryStream> {
    stream1: Fuse<IntoStream<St1>>,
    stream2: Fuse<IntoStream<St2>>,
    queued1: Option<Result<St1::Ok, St1::Error>>,
    queued2: Option<Result<St2::Ok, St2::Error>>,
}

impl<St1, St2> Unpin for TryZip<St1, St2>
where
    St1: TryStream,
    Fuse<IntoStream<St1>>: Unpin,
    St2: TryStream,
    Fuse<IntoStream<St2>>: Unpin,
{}

impl<St1: TryStream, St2: TryStream> TryZip<St1, St2> {
    unsafe_pinned!(stream1: Fuse<IntoStream<St1>>);
    unsafe_pinned!(stream2: Fuse<IntoStream<St2>>);
    unsafe_unpinned!(queued1: Option<Result<St1::Ok, St1::Error>>);
    unsafe_unpinned!(queued2: Option<Result<St2::Ok, St2::Error>>);

    pub(super) fn new(stream1: St1, stream2: St2) -> TryZip<St1, St2> {
        TryZip {
            stream1: stream1.into_stream().fuse(),
            stream2: stream2.into_stream().fuse(),
            queued1: None,
            queued2: None,
        }
    }

    /// Acquires a reference to the underlying streams that this combinator is
    /// pulling from.
    pub fn get_ref(&self) -> (&St1, &St2) {
        (self.stream1.get_ref().get_ref(), self.stream2.get_ref().get_ref())
    }

    /// Acquires a mutable reference to the underlying streams that this
    /// combinator is pulling from.
    ///
    /// Note that care must be taken to avoid tampering with the state of the
    /// stream which may otherwise confuse this combinator.
    pub fn get_mut(&mut self) -> (&mut St1, &mut St2) {
        (self.stream1.get_mut().get_mut(), self.stream2.get_mut().get_mut())
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
        (Pin::new(stream1.get_mut().get_mut()), Pin::new(stream2.get_mut().get_mut()))
    }

    /// Consumes this combinator, returning the underlying streams.
    ///
    /// Note that this may discard intermediate state of this combinator, so
    /// care should be taken to avoid losing resources when this is called.
    pub fn into_inner(self) -> (St1, St2) {
        (self.stream1.into_inner().into_inner(), self.stream2.into_inner().into_inner())
    }
}

impl<St1, St2> FusedStream for TryZip<St1, St2>
    where St1: TryStream, St2: TryStream,
{
    fn is_terminated(&self) -> bool {
        self.stream1.is_terminated() && self.stream2.is_terminated()
    }
}

impl<St1, St2> TryStream for TryZip<St1, St2>
where
    St1: TryStream,
    St2: TryStream<Error = St1::Error>
{
    type Ok = (St1::Ok, St2::Ok);
    type Error = St1::Error;

    fn try_poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Self::Ok, Self::Error>>> {
        if self.queued1.is_none() {
            match self.as_mut().stream1().try_poll_next(cx) {
                Poll::Ready(Some(item1)) => *self.as_mut().queued1() = Some(item1),
                Poll::Ready(None) | Poll::Pending => {}
            }
        }
        if self.queued2.is_none() {
            match self.as_mut().stream2().try_poll_next(cx) {
                Poll::Ready(Some(item2)) => *self.as_mut().queued2() = Some(item2),
                Poll::Ready(None) | Poll::Pending => {}
            }
        }

        if self.queued1.is_some() && self.queued2.is_some() {
            let pair = (self.as_mut().queued1().take().unwrap(),
                        self.as_mut().queued2().take().unwrap());

            let pair = match pair {
                (Ok(a), Ok(b)) => Ok((a, b)),
                (Ok(_), Err(b)) => Err(b),
                (Err(a), Ok(_)) => Err(a),
                (Err(a), Err(_)) => Err(a),
            };

            Poll::Ready(Some(pair))
        } else if self.stream1.is_done() || self.stream2.is_done() {
            Poll::Ready(None)
        } else {
            Poll::Pending
        }
    }
}
