use anchor_experiment::MovePinned;
use futures_core::{Future, FutureMove, IntoFuture, Poll};
use super::chain::Chain;

/// Future for the `then` combinator, chaining computations on the end of
/// another future regardless of its outcome.
///
/// This is created by the `Future::then` method.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct Then<A, B, F> where A: Future, B: IntoFuture {
    state: Chain<A, B::Future, F>,
}

pub fn new<A, B, F>(future: A, f: F) -> Then<A, B, F>
    where A: Future,
          B: IntoFuture,
{
    Then {
        state: Chain::new(future, f),
    }
}

impl<A, B, F> Future for Then<A, B, F>
    where A: Future,
          B: IntoFuture,
          F: FnOnce(Result<A::Item, A::Error>) -> B,
{
    type Item = B::Item;
    type Error = B::Error;

    unsafe fn poll_unsafe(&mut self) -> Poll<B::Item, B::Error> {
        self.state.poll_unsafe(|a, f| {
            Ok(Err(f(a).into_future()))
        })
    }
}

impl<A, B, F> FutureMove for Then<A, B, F>
    where A: FutureMove,
          B: IntoFuture,
          B::Future: FutureMove,
          F: FnOnce(Result<A::Item, A::Error>) -> B + MovePinned,
{
    fn poll_move(&mut self) -> Poll<B::Item, B::Error> {
        unsafe { self.poll_unsafe() }
    }
}
