use {Future, PollResult, Callback};
use executor::{Executor, DEFAULT};
use util;

pub struct Map<A, F> {
    future: A,
    f: Option<F>,
}

pub fn new<A, F>(future: A, f: F) -> Map<A, F> {
    Map {
        future: future,
        f: Some(f),
    }
}

impl<U, A, F> Future for Map<A, F>
    where A: Future,
          F: FnOnce(A::Item) -> U + Send + 'static,
          U: Send + 'static,
{
    type Item = U;
    type Error = A::Error;

    fn schedule<G>(&mut self, g: G)
        where G: FnOnce(PollResult<U, A::Error>) + Send + 'static
    {
        let f = match util::opt2poll(self.f.take()) {
            Ok(f) => f,
            Err(e) => return g(Err(e)),
        };
        self.future.schedule(|result| {
            let res = result.and_then(|e| util::recover(|| f(e)));
            DEFAULT.execute(|| g(res))
        })
    }

    fn schedule_boxed(&mut self, cb: Box<Callback<U, A::Error>>) {
        self.schedule(|r| cb.call(r));
    }
}
