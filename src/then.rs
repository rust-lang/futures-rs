use {Future, IntoFuture, PollError, Callback, PollResult};
use util;
use chain::Chain;

pub struct Then<A, B, F> where B: IntoFuture {
    state: Chain<A, B::Future, F>,
}

pub fn new<A, B, F>(future: A, f: F) -> Then<A, B, F>
    where A: Future,
          B: IntoFuture,
          F: Send + 'static,
{
    Then {
        state: Chain::new(future, f),
    }
}

impl<A, B, F> Future for Then<A, B, F>
    where A: Future,
          B: IntoFuture,
          F: FnOnce(Result<A::Item, A::Error>) -> B + Send + 'static,
{
    type Item = B::Item;
    type Error = B::Error;

    fn schedule<G>(&mut self, g: G)
        where G: FnOnce(PollResult<B::Item, B::Error>) + Send + 'static
    {
        self.state.schedule(g, |a, f| {
            let ret = match a {
                Ok(e) => util::recover(|| f(Ok(e))),
                Err(PollError::Other(e)) => util::recover(|| f(Err(e))),
                Err(PollError::Panicked(e)) => Err(PollError::Panicked(e)),
                Err(PollError::Canceled) => Err(PollError::Canceled),
            };
            ret.map(|b| Err(b.into_future()))
        })
    }

    fn schedule_boxed(&mut self, cb: Box<Callback<B::Item, B::Error>>) {
        self.schedule(|r| cb.call(r))
    }
}
