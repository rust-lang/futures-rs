use core::pin::Pin;
use futures_core::iteration;
use futures_core::stream::{FusedStream, Stream, TryStream};
use futures_core::task::{Context, Poll};
#[cfg(feature = "sink")]
use futures_sink::Sink;
use pin_utils::{unsafe_pinned, unsafe_unpinned};

/// Stream for the [`try_flatten`](super::TryStreamExt::try_flatten) method.
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct TryFlatten<St>
where
    St: TryStream,
{
    stream: St,
    next: Option<St::Ok>,
    yield_after: iteration::Limit,
}

impl<St> Unpin for TryFlatten<St>
where
    St: TryStream + Unpin,
    St::Ok: Unpin,
{
}

impl<St> TryFlatten<St>
where
    St: TryStream,
{
    unsafe_pinned!(stream: St);
    unsafe_pinned!(next: Option<St::Ok>);
    unsafe_unpinned!(yield_after: iteration::Limit);

    fn split_borrows(
        self: Pin<&mut Self>,
    ) -> (
        Pin<&mut St>,
        Pin<&mut Option<St::Ok>>,
        &mut iteration::Limit,
    ) {
        unsafe {
            let this = self.get_unchecked_mut();
            (
                Pin::new_unchecked(&mut this.stream),
                Pin::new_unchecked(&mut this.next),
                &mut this.yield_after,
            )
        }
    }
}

impl<St> TryFlatten<St>
where
    St: TryStream,
    St::Ok: TryStream,
    <St::Ok as TryStream>::Error: From<St::Error>,
{
    pub(super) fn new(stream: St) -> Self {
        Self {
            stream,
            next: None,
            yield_after: crate::DEFAULT_YIELD_AFTER_LIMIT,
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
    pub fn get_pin_mut(self: Pin<&mut Self>) -> Pin<&mut St> {
        self.stream()
    }

    /// Consumes this combinator, returning the underlying stream.
    ///
    /// Note that this may discard intermediate state of this combinator, so
    /// care should be taken to avoid losing resources when this is called.
    pub fn into_inner(self) -> St {
        self.stream
    }

    stream_method_yield_after_every! {
        #[doc = "the underlying stream or a yet unconsumed stream yielded by it"]
        #[doc = "the underlying stream keeps yielding streams that immediately poll empty,"]
    }
}

impl<St> FusedStream for TryFlatten<St>
where
    St: TryStream + FusedStream,
    St::Ok: TryStream,
    <St::Ok as TryStream>::Error: From<St::Error>,
{
    fn is_terminated(&self) -> bool {
        self.next.is_none() && self.stream.is_terminated()
    }
}

impl<St> Stream for TryFlatten<St>
where
    St: TryStream,
    St::Ok: TryStream,
    <St::Ok as TryStream>::Error: From<St::Error>,
{
    type Item = Result<<St::Ok as TryStream>::Ok, <St::Ok as TryStream>::Error>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let (mut stream, mut next, yield_after) = self.split_borrows();
        poll_loop! { yield_after, cx, {
            if next.as_ref().is_none() {
                match ready!(stream.as_mut().try_poll_next(cx)?) {
                    Some(e) => next.as_mut().set(Some(e)),
                    None => return Poll::Ready(None),
                }
            }

            if let Some(item) = ready!(next.as_mut()
                .as_pin_mut()
                .unwrap()
                .try_poll_next(cx)?)
            {
                return Poll::Ready(Some(Ok(item)));
            } else {
                next.as_mut().set(None);
            }
        }}
    }
}

// Forwarding impl of Sink from the underlying stream
#[cfg(feature = "sink")]
impl<S, Item> Sink<Item> for TryFlatten<S>
where
    S: TryStream + Sink<Item>,
{
    type Error = <S as Sink<Item>>::Error;

    delegate_sink!(stream, Item);
}
