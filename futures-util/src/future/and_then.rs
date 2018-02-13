use anchor_experiment::MovePinned;
use futures_core::{Future, FutureMove, IntoFuture, Poll};

use super::chain::Chain;

/// Future for the `and_then` combinator, chaining a computation onto the end of
/// another future which completes successfully.
///
/// This is created by the `Future::and_then` method.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct AndThen<A, B, F> where A: Future, B: IntoFuture {
    state: Chain<A, B::Future, F>,
}

pub fn new<A, B, F>(future: A, f: F) -> AndThen<A, B, F>
    where A: Future,
          B: IntoFuture,
{
    AndThen {
        state: Chain::new(future, f),
    }
}

impl<A, B, F> Future for AndThen<A, B, F>
    where A: Future,
          B: IntoFuture<Error=A::Error>,
          F: FnOnce(A::Item) -> B,
{
    type Item = B::Item;
    type Error = B::Error;

    unsafe fn poll_unsafe(&mut self) -> Poll<B::Item, B::Error> {
        self.state.poll_unsafe(|result, f| {
            result.map(|e| {
                Err(f(e).into_future())
            })
        })
    }
}

impl<A, B, F> FutureMove for AndThen<A, B, F>
    where A: FutureMove,
          B: IntoFuture<Error=A::Error>,
          B::Future: FutureMove,
          F: FnOnce(A::Item) -> B + MovePinned,
{
    fn poll_move(&mut self) -> Poll<B::Item, B::Error> {
        unsafe { self.poll_unsafe()  }
    }
}
