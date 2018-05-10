use core::fmt;

use futures_core::{Future, Poll, Stream};
use futures_core::task;

#[cfg(feature = "nightly")]
use core::mem::PinMut;

/// Future for the `flatten_stream` combinator, flattening a
/// future-of-a-stream to get just the result of the final stream as a stream.
///
/// This is created by the `Future::flatten_stream` method.
#[must_use = "streams do nothing unless polled"]
pub struct FlattenStream<F: Future> {
    state: State<F>
}

impl<F> fmt::Debug for FlattenStream<F>
    where F: Future + fmt::Debug,
          F::Output: fmt::Debug,
{
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("FlattenStream")
            .field("state", &self.state)
            .finish()
    }
}

pub fn new<F: Future>(f: F) -> FlattenStream<F> {
    FlattenStream {
        state: State::Future(f)
    }
}

#[derive(Debug)]
enum State<F: Future> {
    // future is not yet called or called and not ready
    Future(F),
    // future resolved to Stream
    Stream(F::Output),
}

#[cfg(feature = "nightly")]
impl<F> Stream for FlattenStream<F>
    where F: Future,
          F::Output: Stream,
{
    type Item = <F::Output as Stream>::Item;

    fn poll_next(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<Option<Self::Item>> {
        loop {
            // safety: data is never moved via the resulting &mut reference
            let stream = match unsafe { PinMut::get_mut(self.reborrow()) }.state {
                State::Future(ref mut f) => {
                    // safety: the future we're re-pinning here will never be moved;
                    // it will just be polled, then dropped in place
                    match unsafe { PinMut::new_unchecked(f) }.poll(cx) {
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
                State::Stream(ref mut s) => {
                    // safety: the stream we're repinning here will never be moved;
                    // it will just be polled, then dropped in place
                    return unsafe { PinMut::new_unchecked(s) }.poll_next(cx);
                }
            };

            unsafe {
                // safety: we use the &mut only for an assignment, which causes
                // only an in-place drop
                PinMut::get_mut(self.reborrow()).state = State::Stream(stream);
            }
        }
    }
}


#[cfg(not(feature = "nightly"))]
impl<F> Stream for FlattenStream<F>
    where F: Future + ::futures_core::Unpin,
          F::Output: Stream + ::futures_core::Unpin,
{
    type Item = <F::Output as Stream>::Item;

    fn poll_next_mut(&mut self, cx: &mut task::Context) -> Poll<Option<Self::Item>> {
        loop {
            let stream = match self.state {
                State::Future(ref mut f) => {
                    match f.poll_unpin(cx) {
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
                State::Stream(ref mut s) => {
                    return s.poll_next_mut(cx);
                }
            };
            self.state = State::Stream(stream)
        }
    }

    unpinned_poll_next!();
}

#[cfg(not(feature = "nightly"))]
unsafe impl<F: Future> ::futures_core::Unpin for FlattenStream<F> {}
