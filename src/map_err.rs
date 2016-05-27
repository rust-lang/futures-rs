use {PollResult, Future, Callback, PollError};
use executor::{Executor, DEFAULT};
use util;

pub struct MapErr<A, F> {
    future: A,
    f: Option<F>,
}

pub fn new<A, F>(future: A, f: F) -> MapErr<A, F> {
    MapErr {
        future: future,
        f: Some(f),
    }
}

impl<U, A, F> Future for MapErr<A, F>
    where A: Future,
          F: FnOnce(A::Error) -> U + Send + 'static,
          U: Send + 'static,
{
    type Item = A::Item;
    type Error = U;

    fn schedule<G>(&mut self, g: G)
        where G: FnOnce(PollResult<A::Item, U>) + Send + 'static
    {
        let f = match util::opt2poll(self.f.take()) {
            Ok(f) => f,
            Err(e) => return DEFAULT.execute(|| g(Err(e))),
        };

        self.future.schedule(|result| {
            let r = match (result, f) {
                (Err(PollError::Other(e)), f) => {
                    util::recover(|| f(e)).and_then(|e| Err(PollError::Other(e)))
                }
                (Err(PollError::Panicked(e)), _) => Err(PollError::Panicked(e)),
                (Err(PollError::Canceled), _) => Err(PollError::Canceled),
                (Ok(e), _) => Ok(e),
            };
            DEFAULT.execute(|| g(r))
        })
    }

    fn schedule_boxed(&mut self, cb: Box<Callback<A::Item, U>>) {
        self.schedule(|r| cb.call(r));
    }
}

