use {PollResult, Future, Callback, PollError, FutureResult};
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
fn map_err<T, U, E, F>(result: PollResult<T, E>, f: F) -> PollResult<T, U>
    where F: FnOnce(E) -> U + Send + 'static,
          T: Send + 'static,
          E: Send + 'static,
{
    match result {
        Err(PollError::Other(e)) => {
            util::recover(|| f(e)).and_then(|e| Err(PollError::Other(e)))
        }
        Err(PollError::Panicked(e)) => Err(PollError::Panicked(e)),
        Err(PollError::Canceled) => Err(PollError::Canceled),
        Ok(e) => Ok(e),
    }
}

impl<U, A, F> Future for MapErr<A, F>
    where A: Future,
          F: FnOnce(A::Error) -> U + Send + 'static,
          U: Send + 'static,
{
    type Item = A::Item;
    type Error = U;

    fn poll(&mut self) -> Option<PollResult<A::Item, U>> {
        let f = match util::opt2poll(self.f.take()) {
            Ok(f) => f,
            Err(e) => return Some(Err(e)),
        };
        self.future.poll().map(|res| map_err(res, f))
    }

    fn cancel(&mut self) {
        self.future.cancel()
    }

    fn await(&mut self) -> FutureResult<A::Item, U> {
        let f = try!(util::opt2poll(self.f.take()));
        self.future.await().map_err(|e| e.map(f))
    }

    fn schedule<G>(&mut self, g: G)
        where G: FnOnce(PollResult<A::Item, U>) + Send + 'static
    {
        match util::opt2poll(self.f.take()) {
            Ok(f) => self.future.schedule(|result| g(map_err(result, f))),
            Err(e) => g(Err(e)),
        }
    }

    fn schedule_boxed(&mut self, cb: Box<Callback<A::Item, U>>) {
        self.schedule(|r| cb.call(r));
    }
}

