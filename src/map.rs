use {Future, PollResult, Callback};
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

fn map<T, U, E, F>(result: PollResult<T, E>, f: F) -> PollResult<U, E>
    where F: FnOnce(T) -> U + Send + 'static,
          T: Send + 'static,
{
    match result {
        Ok(e) => util::recover(|| f(e)),
        Err(e) => Err(e),
    }
}

impl<U, A, F> Future for Map<A, F>
    where A: Future,
          F: FnOnce(A::Item) -> U + Send + 'static,
          U: Send + 'static,
{
    type Item = U;
    type Error = A::Error;

    // fn poll(&mut self) -> Option<PollResult<U, A::Error>> {
    //     let f = match util::opt2poll(self.f.take()) {
    //         Ok(f) => f,
    //         Err(e) => return Some(Err(e)),
    //     };
    //     match self.future.poll() {
    //         Some(res) => Some(map(res, f)),
    //         None => {
    //             self.f = Some(f);
    //             None
    //         }
    //     }
    // }

    // fn cancel(&mut self) {
    //     self.future.cancel()
    // }

    // fn await(&mut self) -> FutureResult<U, A::Error> {
    //     let f = try!(util::opt2poll(self.f.take()));
    //     self.future.await().map(f)
    // }

    fn schedule<G>(&mut self, g: G)
        where G: FnOnce(PollResult<U, A::Error>) + Send + 'static
    {
        match util::opt2poll(self.f.take()) {
            Ok(f) => self.future.schedule(|result| g(map(result, f))),
            Err(e) => g(Err(e)),
        }
    }

    fn schedule_boxed(&mut self, cb: Box<Callback<U, A::Error>>) {
        self.schedule(|r| cb.call(r));
    }
}
