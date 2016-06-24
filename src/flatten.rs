use std::sync::Arc;

use {Future, IntoFuture, Wake, PollResult};
use chain::Chain;

/// Future for the `flatten` combinator, flattening a future-of-a-future to just
/// the result of the final future.
///
/// This is created by this `Future::flatten` method.
pub struct Flatten<A> where A: Future, A::Item: IntoFuture {
    state: Chain<A, <A::Item as IntoFuture>::Future, ()>,
}

pub fn new<A>(future: A) -> Flatten<A>
    where A: Future,
          A::Item: IntoFuture,
{
    Flatten {
        state: Chain::new(future, ()),
    }
}

impl<A> Future for Flatten<A>
    where A: Future,
          A::Item: IntoFuture,
          <<A as Future>::Item as IntoFuture>::Error: From<<A as Future>::Error>
{
    type Item = <<A as Future>::Item as IntoFuture>::Item;
    type Error = <<A as Future>::Item as IntoFuture>::Error;

    fn poll(&mut self) -> Option<PollResult<Self::Item, Self::Error>> {
        self.state.poll(|a, ()| {
            match a {
                Ok(item) => Ok(Err(item.into_future())),
                Err(e) => Err(e.map(From::from)),
            }
        })
    }

    fn schedule(&mut self, wake: Arc<Wake>) {
        self.state.schedule(wake)
    }

    fn tailcall(&mut self)
                -> Option<Box<Future<Item=Self::Item, Error=Self::Error>>> {
        self.state.tailcall()
    }
}
