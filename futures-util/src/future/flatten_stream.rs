use core::fmt;
use core::pin::Pin;
use futures_core::future::Future;
use futures_core::stream::{FusedStream, Stream};
use futures_core::task::{LocalWaker, Poll};

/// Future for the `flatten_stream` combinator, flattening a
/// future-of-a-stream to get just the result of the final stream as a stream.
///
/// This is created by the `Future::flatten_stream` method.
#[must_use = "streams do nothing unless polled"]
pub struct FlattenStream<Fut: Future> {
    state: State<Fut>
}

impl<Fut: Future> FlattenStream<Fut> {
    pub(super) fn new(future: Fut) -> FlattenStream<Fut> {
        FlattenStream {
            state: State::Future(future)
        }
    }
}

impl<Fut> fmt::Debug for FlattenStream<Fut>
    where Fut: Future + fmt::Debug,
          Fut::Output: fmt::Debug,
{
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("FlattenStream")
            .field("state", &self.state)
            .finish()
    }
}

#[derive(Debug)]
enum State<Fut: Future> {
    // future is not yet called or called and not ready
    Future(Fut),
    // future resolved to Stream
    Stream(Fut::Output),
}

impl<Fut> FusedStream for FlattenStream<Fut>
    where Fut: Future,
          Fut::Output: Stream + FusedStream,
{
    fn is_terminated(&self) -> bool {
        match &self.state {
            State::Future(_) => false,
            State::Stream(stream) => stream.is_terminated(),
        }
    }
}

impl<Fut> Stream for FlattenStream<Fut>
    where Fut: Future,
          Fut::Output: Stream,
{
    type Item = <Fut::Output as Stream>::Item;

    fn poll_next(mut self: Pin<&mut Self>, lw: &LocalWaker) -> Poll<Option<Self::Item>> {
        loop {
            // safety: data is never moved via the resulting &mut reference
            let stream = match &mut unsafe { Pin::get_unchecked_mut(self.as_mut()) }.state {
                State::Future(f) => {
                    // safety: the future we're re-pinning here will never be moved;
                    // it will just be polled, then dropped in place
                    match unsafe { Pin::new_unchecked(f) }.poll(lw) {
                        Poll::Pending => {
                            // State is not changed, early return.
                            return Poll::Pending
                        },
                        Poll::Ready(stream) => {
                            // Future resolved to stream.
                            // We do not return, but poll that
                            // stream in the next loop iteration.
                            stream
                        }
                    }
                }
                State::Stream(s) => {
                    // safety: the stream we're repinning here will never be moved;
                    // it will just be polled, then dropped in place
                    return unsafe { Pin::new_unchecked(s) }.poll_next(lw);
                }
            };

            unsafe {
                // safety: we use the &mut only for an assignment, which causes
                // only an in-place drop
                Pin::get_unchecked_mut(self.as_mut()).state = State::Stream(stream);
            }
        }
    }
}
