use {Future, IntoFuture, Poll};
use future::InfallibleFuture;
use super::chain::Chain;
use super::infallible::InfallibleCastErr;
use never::{InfallibleResultExt, Never};

/// Future for the `then_infallible` combinator, chaining computations on the end of
/// an infallible future.
///
/// This is created by the `Future::then_infallible` method.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct ThenInfallible<A, B, F> where A: InfallibleFuture, B: IntoFuture {
    state: Chain<InfallibleCastErr<A, Never>, B::Future, F>,
}

pub fn new<A, B, F>(future: A, f: F) -> ThenInfallible<A, B, F>
    where A: InfallibleFuture,
          B: IntoFuture,
{
    ThenInfallible {
        state: Chain::new(InfallibleCastErr::new(future), f),
    }
}

impl<A, B, F> Future for ThenInfallible<A, B, F>
    where A: InfallibleFuture,
          B: IntoFuture,
          F: FnOnce(A::Item) -> B,
{
    type Item = B::Item;
    type Error = B::Error;

    fn poll(&mut self) -> Poll<B::Item, B::Error> {
        self.state.poll(|a, f| {
            Ok(Err(f(a.infallible()).into_future()))
        })
    }
}
