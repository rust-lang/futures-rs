use {Future, IntoFuture, Callback, PollResult, PollError};
use util;
use chain::Chain;

pub struct OrElse<A, B, F> where B: IntoFuture {
    state: Chain<A, B::Future, F>,
}

pub fn new<A, B, F>(future: A, f: F) -> OrElse<A, B, F>
    where A: Future,
          B: IntoFuture<Item=A::Item>,
          F: Send + 'static,
{
    OrElse {
        state: Chain::new(future, f),
    }
}

fn or_else<A, B, F>(a: PollResult<A::Item, A::Error>, f: F)
                    -> PollResult<Result<B::Item, B::Future>, B::Error>
    where A: Future,
          B: IntoFuture<Item=A::Item>,
          F: FnOnce(A::Error) -> B + Send + 'static,
{
    match a {
        Ok(item) => Ok(Ok(item)),
        Err(PollError::Panicked(d)) => Err(PollError::Panicked(d)),
        Err(PollError::Canceled) => Err(PollError::Canceled),
        Err(PollError::Other(e)) => {
            util::recover(|| f(e)).map(|e| Err(e.into_future()))
        }
    }
}

impl<A, B, F> Future for OrElse<A, B, F>
    where A: Future,
          B: IntoFuture<Item=A::Item>,
          F: FnOnce(A::Error) -> B + Send + 'static,
{
    type Item = B::Item;
    type Error = B::Error;

    // fn poll(&mut self) -> Option<PollResult<B::Item, B::Error>> {
    //     self.state.poll(or_else::<A, B, F>)
    // }

    fn cancel(&mut self) {
        self.state.cancel()
    }

    // fn await(&mut self) -> FutureResult<B::Item, B::Error> {
    //     self.state.await(or_else::<A, B, F>)
    // }

    fn schedule<G>(&mut self, g: G)
        where G: FnOnce(PollResult<B::Item, B::Error>) + Send + 'static
    {
        self.state.schedule(g, or_else::<A, B, F>)
    }

    fn schedule_boxed(&mut self, cb: Box<Callback<B::Item, B::Error>>) {
        // TODO: wut? UFCS?
        Future::schedule(self, |r| cb.call(r))
    }
}
