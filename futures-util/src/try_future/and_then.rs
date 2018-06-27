use core::mem::PinMut;

use futures_core::{Future, Poll, TryFuture};
use futures_core::task;

/// Future for the `and_then` combinator, chaining a computation onto the end of
/// another future which completes successfully.
///
/// This is created by the `Future::and_then` method.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct AndThen<A, B, F> {
    state: State<A, B, F>,
}

#[derive(Debug)]
enum State<Fut1, Fut2, F> {
    First(Fut1, Option<F>),
    Second(Fut2),
}

pub fn new<A, B, F>(future: A, f: F) -> AndThen<A, B, F> {
    AndThen {
        state: State::First(future, Some(f)),
    }
}

impl<A, B, F> Future for AndThen<A, B, F>
    where A: TryFuture,
          B: TryFuture<Error = A::Error>,
          F: FnOnce(A::Item) -> B,
{
    type Output = Result<B::Item, B::Error>;

    fn poll(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<Self::Output> {
        loop {
            // Safe to use `get_mut_unchecked` here because we don't move out
            let fut2 = match &mut unsafe { PinMut::get_mut_unchecked(self.reborrow()) }.state {
                State::First(fut1, data) => {
                    // safe to create a new `PinMut` because `fut1` will never move
                    // before it's dropped.
                    match unsafe { PinMut::new_unchecked(fut1) }.try_poll(cx) {
                        Poll::Pending => return Poll::Pending,
                        Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                        Poll::Ready(Ok(v)) => {
                            (data.take().unwrap())(v)
                        }
                    }
                }
                State::Second(fut2) => {
                    // Safe to create a new `PinMut` because `fut2` will never move
                    // before it's dropped; once we're in `Chain::Second` we stay
                    // there forever.
                    return unsafe { PinMut::new_unchecked(fut2) }.try_poll(cx)
                }
            };

            // Safe because we're using the `&mut` to do an assignment, not for moving out
            unsafe {
                // Note: it's safe to move the `fut2` here because we haven't yet polled it
                PinMut::get_mut_unchecked(self.reborrow()).state = State::Second(fut2);
            }
        }
    }
}
