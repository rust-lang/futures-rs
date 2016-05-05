use {Future, PollResult, Callback, IntoFuture};
use util;

pub struct Lazy<F, R> {
    inner: _Lazy<F, R>
}

enum _Lazy<F, R> {
    First(Option<F>),
    Second(R),
}

pub fn lazy<F, R>(f: F) -> Lazy<F, R::Future>
    where F: FnOnce() -> R + Send + 'static,
          R: IntoFuture
{
    Lazy { inner: _Lazy::First(Some(f)) }
}

impl<F, R> Future for Lazy<F, R::Future>
    where F: FnOnce() -> R + Send + 'static,
          R: IntoFuture,
{
    type Item = R::Item;
    type Error = R::Error;

    // fn poll(&mut self) -> Option<PollResult<R::Item, R::Error>> {
    //     match self.get() {
    //         Ok(f) => f.poll(),
    //         Err(e) => Some(Err(e)),
    //     }
    // }

    // fn await(&mut self) -> FutureResult<R::Item, R::Error> {
    //     try!(self.get()).await()
    // }

    // fn cancel(&mut self) {
    //     if let _Lazy::Second(ref mut f) = self.inner {
    //         return f.cancel()
    //     }
    //     self.inner = _Lazy::Canceled;
    // }

    fn schedule<G>(&mut self, g: G)
        where G: FnOnce(PollResult<R::Item, R::Error>) + Send + 'static
    {
        let mut future = match self.inner {
            _Lazy::First(ref mut f) => {
                let f = util::opt2poll(f.take());
                match f.and_then(util::recover) {
                    Ok(f) => f.into_future(),
                    Err(e) => return g(Err(e)),
                }
            }
            _Lazy::Second(ref mut f) => return f.schedule(g),
        };
        future.schedule(g);
        self.inner = _Lazy::Second(future);
    }

    fn schedule_boxed(&mut self, cb: Box<Callback<R::Item, R::Error>>) {
        // TODO: wut? UFCS?
        Future::schedule(self, |r| cb.call(r))
    }
}
