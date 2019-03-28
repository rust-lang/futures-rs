use core::pin::Pin;
use futures_core::future::{TryFuture};
use futures_core::stream::{Stream, TryStream};
use futures_core::task::{Waker, Poll};
use pin_utils::{unsafe_pinned, unsafe_unpinned};

/// Stream for the [`try_filter_map`](super::TryStreamExt::try_filter_map)
/// combinator.
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct TryFilterMap<St, Fut, F> {
    stream: St,
    f: F,
    pending: Option<Fut>,
}

impl<St, Fut, F> Unpin for TryFilterMap<St, Fut, F>
    where St: Unpin, Fut: Unpin,
{}

impl<St, Fut, F> TryFilterMap<St, Fut, F> {
    unsafe_pinned!(stream: St);
    unsafe_unpinned!(f: F);
    unsafe_pinned!(pending: Option<Fut>);

    pub(super) fn new(stream: St, f: F) -> Self {
        TryFilterMap { stream, f, pending: None }
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

    /// Consumes this combinator, returning the underlying stream.
    ///
    /// Note that this may discard intermediate state of this combinator, so
    /// care should be taken to avoid losing resources when this is called.
    pub fn into_inner(self) -> St {
        self.stream
    }
}

impl<St, Fut, F, T> Stream for TryFilterMap<St, Fut, F>
    where St: TryStream,
          Fut: TryFuture<Ok = Option<T>, Error = St::Error>,
          F: FnMut(St::Ok) -> Fut,
{
    type Item = Result<T, St::Error>;

    fn poll_next(
        mut self: Pin<&mut Self>,
        waker: &Waker,
    ) -> Poll<Option<Result<T, St::Error>>> {
        loop {
            if self.pending.is_none() {
                let item = match ready!(self.as_mut().stream().try_poll_next(waker)) {
                    Some(Ok(x)) => x,
                    Some(Err(e)) => return Poll::Ready(Some(Err(e))),
                    None => return Poll::Ready(None),
                };
                let fut = (self.as_mut().f())(item);
                self.as_mut().pending().set(Some(fut));
            }

            let result = ready!(self.as_mut().pending().as_pin_mut().unwrap().try_poll(waker));
            self.as_mut().pending().set(None);
            match result {
                Ok(Some(x)) => return Poll::Ready(Some(Ok(x))),
                Err(e) => return Poll::Ready(Some(Err(e))),
                Ok(None) => {},
            }
        }
    }
}

