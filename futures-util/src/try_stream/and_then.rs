use core::pin::Pin;
use futures_core::future::TryFuture;
use futures_core::stream::{Stream, TryStream};
use futures_core::task::{Context, Poll};
use futures_sink::Sink;
use pin_utils::{unsafe_pinned, unsafe_unpinned};

/// Stream for the [`and_then`](super::TryStreamExt::and_then) method.
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct AndThen<St, Fut, F> {
    stream: St,
    future: Option<Fut>,
    f: F,
}

impl<St: Unpin, Fut: Unpin, F> Unpin for AndThen<St, Fut, F> {}

impl<St, Fut, F> AndThen<St, Fut, F>
    where St: TryStream,
          F: FnMut(St::Ok) -> Fut,
          Fut: TryFuture<Error = St::Error>,
{
    unsafe_pinned!(stream: St);
    unsafe_pinned!(future: Option<Fut>);
    unsafe_unpinned!(f: F);

    pub(super) fn new(stream: St, f: F) -> Self {
        Self { stream, future: None, f }
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
    pub fn get_pin_mut<'a>(self: Pin<&'a mut Self>) -> Pin<&'a mut St> {
        self.stream()
    }

    /// Consumes this combinator, returning the underlying stream.
    ///
    /// Note that this may discard intermediate state of this combinator, so
    /// care should be taken to avoid losing resources when this is called.
    pub fn into_inner(self) -> St {
        self.stream
    }
}

impl<St, Fut, F> Stream for AndThen<St, Fut, F>
    where St: TryStream,
          F: FnMut(St::Ok) -> Fut,
          Fut: TryFuture<Error = St::Error>,
{
    type Item = Result<Fut::Ok, St::Error>;

    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        if self.future.is_none() {
            let item = match ready!(self.as_mut().stream().try_poll_next(cx)?) {
                None => return Poll::Ready(None),
                Some(e) => e,
            };
            let fut = (self.as_mut().f())(item);
            self.as_mut().future().set(Some(fut));
        }

        assert!(self.future.is_some());
        match ready!(self.as_mut().future().as_pin_mut().unwrap().try_poll(cx)) {
            Ok(e) => {
                self.as_mut().future().set(None);
                Poll::Ready(Some(Ok(e)))
            }
            Err(e) => {
                self.as_mut().future().set(None);
                Poll::Ready(Some(Err(e)))
            }
        }
    }
}

// Forwarding impl of Sink from the underlying stream
impl<S, Fut, F, Item> Sink<Item> for AndThen<S, Fut, F>
    where S: TryStream + Sink<Item>,
          F: FnMut(S::Ok) -> Fut,
          Fut: TryFuture<Error = S::Error>,
{
    type SinkError = S::SinkError;

    delegate_sink!(stream, Item);
}
