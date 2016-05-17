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

impl<A, B, F> Future for OrElse<A, B, F>
    where A: Future,
          B: IntoFuture<Item=A::Item>,
          F: FnOnce(A::Error) -> B + Send + 'static,
{
    type Item = B::Item;
    type Error = B::Error;

    fn schedule<G>(&mut self, g: G)
        where G: FnOnce(PollResult<B::Item, B::Error>) + Send + 'static
    {
        self.state.schedule(g, |a, f| {
            match a {
                Ok(item) => Ok(Ok(item)),
                Err(PollError::Panicked(d)) => Err(PollError::Panicked(d)),
                Err(PollError::Canceled) => Err(PollError::Canceled),
                Err(PollError::Other(e)) => {
                    util::recover(|| f(e)).map(|e| Err(e.into_future()))
                }
            }
        })
    }

    fn schedule_boxed(&mut self, cb: Box<Callback<B::Item, B::Error>>) {
        self.schedule(|r| cb.call(r))
    }
}
