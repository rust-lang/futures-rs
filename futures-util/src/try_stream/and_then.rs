use core::fmt;
use core::pin::Pin;
use futures_core::future::TryFuture;
use futures_core::stream::{Stream, TryStream};
use futures_core::task::{Context, Poll};
#[cfg(feature = "sink")]
use futures_sink::Sink;
use pin_project::{pin_project, unsafe_project};

/// Stream for the [`and_then`](super::TryStreamExt::and_then) method.
#[unsafe_project(Unpin)]
#[must_use = "streams do nothing unless polled"]
pub struct AndThen<St, Fut, F> {
    #[pin]
    stream: St,
    #[pin]
    future: Option<Fut>,
    f: F,
}

impl<St, Fut, F> fmt::Debug for AndThen<St, Fut, F>
where
    St: fmt::Debug,
    Fut: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AndThen")
            .field("stream", &self.stream)
            .field("future", &self.future)
            .finish()
    }
}

impl<St, Fut, F> AndThen<St, Fut, F>
    where St: TryStream,
          F: FnMut(St::Ok) -> Fut,
          Fut: TryFuture<Error = St::Error>,
{
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

impl<St, Fut, F> Stream for AndThen<St, Fut, F>
    where St: TryStream,
          F: FnMut(St::Ok) -> Fut,
          Fut: TryFuture<Error = St::Error>,
{
    type Item = Result<Fut::Ok, St::Error>;

    #[pin_project(self)]
    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        if self.future.is_none() {
            let item = match ready!(self.stream.as_mut().try_poll_next(cx)?) {
                None => return Poll::Ready(None),
                Some(e) => e,
            };
            let fut = (self.f)(item);
            self.future.set(Some(fut));
        }

        let e = ready!(self.future.as_mut().as_pin_mut().unwrap().try_poll(cx));
        self.future.set(None);
        Poll::Ready(Some(e))
    }
}

// Forwarding impl of Sink from the underlying stream
#[cfg(feature = "sink")]
impl<S, Fut, F, Item> Sink<Item> for AndThen<S, Fut, F>
    where S: Sink<Item>,
{
    type Error = S::Error;

    delegate_sink!(stream, Item);
}
