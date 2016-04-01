use {Future, IntoFuture, Callback, PollResult, FutureResult};
use util;
use chain::Chain;

pub struct AndThen<A, B, F> {
    state: Chain<A, B, F>,
}

pub fn new<A, B, F>(future: A, f: F) -> AndThen<A, B, F>
    where A: Future,
          B: Future,
          F: Send + 'static,
{
    AndThen {
        state: Chain::new(future, f),
    }
}

fn and_then<A, B, F>(a: PollResult<A::Item, A::Error>, f: F)
                     -> PollResult<Result<B::Item, B::Future>, B::Error>
    where A: Future,
          B: IntoFuture<Error=A::Error>,
          F: FnOnce(A::Item) -> B + Send + 'static,
{
    let e = try!(a);
    util::recover(|| f(e)).map(|b| Err(b.into_future()))
}

impl<A, B, F> Future for AndThen<A, B::Future, F>
    where A: Future,
          B: IntoFuture<Error=A::Error>,
          F: FnOnce(A::Item) -> B + Send + 'static,
{
    type Item = B::Item;
    type Error = B::Error;

    fn poll(&mut self) -> Option<PollResult<B::Item, B::Error>> {
        self.state.poll(and_then::<A, B, F>)
    }

    fn cancel(&mut self) {
        self.state.cancel()
    }

    fn await(&mut self) -> FutureResult<B::Item, B::Error> {
        self.state.await(and_then::<A, B, F>)
    }

    fn schedule<G>(&mut self, g: G)
        where G: FnOnce(PollResult<B::Item, B::Error>) + Send + 'static
    {
        self.state.schedule(g, and_then::<A, B, F>)
    }

    fn schedule_boxed(&mut self, cb: Box<Callback<B::Item, B::Error>>) {
        // TODO: wut? UFCS?
        Future::schedule(self, |r| cb.call(r))
    }
}
