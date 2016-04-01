use {Future, IntoFuture, Callback, PollResult, FutureResult, PollError};
use chain::Chain;

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

fn map<A>(a: PollResult<A::Item, A::Error>, (): ())
          -> PollResult<Result<<A::Item as IntoFuture>::Item,
                               <A::Item as IntoFuture>::Future>,
                        <A::Item as IntoFuture>::Error>
    where A: Future,
          A::Item: IntoFuture,
          <<A as Future>::Item as IntoFuture>::Error: From<<A as Future>::Error>
{
    match a {
        Ok(item) => Ok(Err(item.into_future())),
        Err(PollError::Other(e)) => Err(PollError::Other(From::from(e))),
        Err(PollError::Panicked(e)) => Err(PollError::Panicked(e)),
        Err(PollError::Canceled) => Err(PollError::Canceled),
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
        self.state.poll(map::<A>)
    }

    fn cancel(&mut self) {
        self.state.cancel()
    }

    fn await(&mut self) -> FutureResult<Self::Item, Self::Error> {
        self.state.await(map::<A>)
    }

    fn schedule<G>(&mut self, g: G)
        where G: FnOnce(PollResult<Self::Item, Self::Error>) + Send + 'static
    {
        self.state.schedule(g, map::<A>)
    }

    fn schedule_boxed(&mut self, cb: Box<Callback<Self::Item, Self::Error>>) {
        // TODO: wut? UFCS?
        Future::schedule(self, |r| cb.call(r))
    }
}
