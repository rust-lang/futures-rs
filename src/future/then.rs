use {Future, IntoFuture, Poll};
use super::chain::Chain;
use task::Task;

/// Future for the `then` combinator, chaining computations on the end of
/// another future regardless of its outcome.
///
/// This is created by the `Future::then` method.
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

    fn poll(&mut self, task: &Task) -> Poll<B::Item, B::Error> {
        self.state.poll(task, |a, f| {
            Ok(Err(f(a).into_future()))
        })
    }
}
