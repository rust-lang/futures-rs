use {Future, IntoFuture, PollError, Callback, PollResult};
use util;
use chain::Chain;

pub struct Then<A, B, F> {
    state: Chain<A, B, F>,
}

pub fn new<A, B, F>(future: A, f: F) -> Then<A, B, F>
    where A: Future,
          B: Future,
          F: Send + 'static,
{
    Then {
        state: Chain::new(future, f),
    }
}

fn then<A, B, F>(a: PollResult<A::Item, A::Error>, f: F)
                 -> PollResult<Result<B::Item, B::Future>, B::Error>
    where A: Future,
          B: IntoFuture,
          F: FnOnce(Result<A::Item, A::Error>) -> B + Send + 'static,
{
    let ret = match a {
        Ok(e) => util::recover(|| f(Ok(e))),
        Err(PollError::Other(e)) => util::recover(|| f(Err(e))),
        Err(PollError::Panicked(e)) => Err(PollError::Panicked(e)),
        Err(PollError::Canceled) => Err(PollError::Canceled),
    };
    ret.map(|b| Err(b.into_future()))
}

impl<A, B, F> Future for Then<A, B::Future, F>
    where A: Future,
          B: IntoFuture,
          F: FnOnce(Result<A::Item, A::Error>) -> B + Send + 'static,
{
    type Item = B::Item;
    type Error = B::Error;

    fn poll(&mut self) -> Option<PollResult<B::Item, B::Error>> {
        self.state.poll(then::<A, B, F>)
    }

    fn cancel(&mut self) {
        self.state.cancel()
    }

    // fn await(&mut self) -> FutureResult<B::Item, B::Error> {
    //     self.state.await(then::<A, B, F>)
    // }

    fn schedule<G>(&mut self, g: G)
        where G: FnOnce(PollResult<B::Item, B::Error>) + Send + 'static
    {
        self.state.schedule(g, then::<A, B, F>)
    }

    fn schedule_boxed(&mut self, cb: Box<Callback<B::Item, B::Error>>) {
        // TODO: wut? UFCS?
        Future::schedule(self, |r| cb.call(r))
    }
}
