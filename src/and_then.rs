use {Future, IntoFuture, Callback, PollResult};
use util;
use chain::Chain;

/// Future for the `and_then` combinator, chaining a computation onto the end of
/// another future which completes successfully.
///
/// This is created by this `Future::and_then` method.
pub struct AndThen<A, B, F> where A: Future, B: IntoFuture {
    state: Chain<A, B::Future, F>,
}

pub fn new<A, B, F>(future: A, f: F) -> AndThen<A, B, F>
    where A: Future,
          B: IntoFuture,
          F: Send + 'static,
{
    AndThen {
        state: Chain::new(future, f),
    }
}

impl<A, B, F> Future for AndThen<A, B, F>
    where A: Future,
          B: IntoFuture<Error=A::Error>,
          F: FnOnce(A::Item) -> B + Send + 'static,
{
    type Item = B::Item;
    type Error = B::Error;

    fn schedule<G>(&mut self, g: G)
        where G: FnOnce(PollResult<B::Item, B::Error>) + Send + 'static
    {
        self.state.schedule(g, |a, f| {
            let e = try!(a);
            util::recover(|| f(e)).map(|b| Err(b.into_future()))
        })
    }

    fn schedule_boxed(&mut self, cb: Box<Callback<B::Item, B::Error>>) {
        self.schedule(|r| cb.call(r))
    }
}
