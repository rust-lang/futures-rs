use {Future, IntoFuture, Callback, PollResult};
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

    fn schedule<G>(&mut self, g: G)
        where G: FnOnce(PollResult<Self::Item, Self::Error>) + Send + 'static
    {
        self.state.schedule(g, |a, ()| {
            match a {
                Ok(item) => Ok(Err(item.into_future())),
                Err(e) => Err(e.map(From::from)),
            }
        })
    }

    fn schedule_boxed(&mut self, cb: Box<Callback<Self::Item, Self::Error>>) {
        self.schedule(|r| cb.call(r))
    }
}
