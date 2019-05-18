use core::fmt;
use core::pin::Pin;
use futures_core::future::TryFuture;
use futures_core::stream::{FusedStream, Stream, TryStream};
use futures_core::task::{Context, Poll};
use pin_utils::unsafe_pinned;

/// Stream for the [`try_flatten_stream`](super::TryFutureExt::try_flatten_stream) method.
#[must_use = "streams do nothing unless polled"]
pub struct TryFlattenStream<Fut>
where
    Fut: TryFuture,
{
    state: State<Fut>
}

impl<Fut: TryFuture> TryFlattenStream<Fut>
where
    Fut: TryFuture,
    Fut::Ok: TryStream<Error = Fut::Error>,
{
    unsafe_pinned!(state: State<Fut>);

    pub(super) fn new(future: Fut) -> Self {
        Self {
            state: State::Future(future)
        }
    }
}

impl<Fut> fmt::Debug for TryFlattenStream<Fut>
where
    Fut: TryFuture + fmt::Debug,
    Fut::Ok: fmt::Debug,
{
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("TryFlattenStream")
            .field("state", &self.state)
            .finish()
    }
}

#[derive(Debug)]
enum State<Fut: TryFuture> {
    // future is not yet called or called and not ready
    Future(Fut),
    // future resolved to Stream
    Stream(Fut::Ok),
    // future resolved to error
    Done,
}

impl<Fut> FusedStream for TryFlattenStream<Fut>
where
    Fut: TryFuture,
    Fut::Ok: TryStream<Error = Fut::Error> + FusedStream,
{
    fn is_terminated(&self) -> bool {
        match &self.state {
            State::Future(_) => false,
            State::Stream(stream) => stream.is_terminated(),
            State::Done => true,
        }
    }
}

impl<Fut> Stream for TryFlattenStream<Fut>
where
    Fut: TryFuture,
    Fut::Ok: TryStream<Error = Fut::Error>,
{
    type Item = Result<<Fut::Ok as TryStream>::Ok, Fut::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            // safety: data is never moved via the resulting &mut reference
            match &mut unsafe { Pin::get_unchecked_mut(self.as_mut()) }.state {
                State::Future(f) => {
                    // safety: the future we're re-pinning here will never be moved;
                    // it will just be polled, then dropped in place
                    match ready!(unsafe { Pin::new_unchecked(f) }.try_poll(cx)) {
                        Ok(stream) => {
                            // Future resolved to stream.
                            // We do not return, but poll that
                            // stream in the next loop iteration.
                            self.as_mut().state().set(State::Stream(stream));
                        }
                        Err(e) => {
                            // Future resolved to error.
                            // We have neither a pollable stream nor a future.
                            self.as_mut().state().set(State::Done);
                            return Poll::Ready(Some(Err(e)));
                        }
                    }
                }
                State::Stream(s) => {
                    // safety: the stream we're repinning here will never be moved;
                    // it will just be polled, then dropped in place
                    return unsafe { Pin::new_unchecked(s) }.try_poll_next(cx);
                }
                State::Done => return Poll::Ready(None),
            }
        }
    }
}
