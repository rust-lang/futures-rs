use {Future, PollError, PollResult, FutureResult, Callback, IntoFuture};
use util;

pub struct Lazy<F, R> {
    inner: _Lazy<F, R>
}

enum _Lazy<F, R> {
    First(Option<F>),
    Second(R),
    Canceled,
}

pub fn lazy<F, R>(f: F) -> Lazy<F, R::Future>
    where F: FnOnce() -> R + Send + 'static,
          R: IntoFuture
{
    Lazy { inner: _Lazy::First(Some(f)) }
}

impl<F, R> Lazy<F, R::Future>
    where F: FnOnce() -> R + Send + 'static,
          R: IntoFuture,
{
    fn get(&mut self) -> PollResult<&mut R::Future, R::Error> {
        let future = match self.inner {
            _Lazy::First(ref mut f) => {
                let f = util::opt2poll(f.take());
                match f.and_then(|f| util::recover(f)) {
                    Ok(f) => f.into_future(),
                    Err(e) => return Err(e),
                }
            }
            _Lazy::Second(ref mut f) => return Ok(f),
            _Lazy::Canceled => return Err(PollError::Canceled),
        };
        self.inner = _Lazy::Second(future);
        match self.inner {
            _Lazy::Second(ref mut f) => Ok(f),
            _ => panic!(),
        }
    }
}

impl<F, R> Future for Lazy<F, R::Future>
    where F: FnOnce() -> R + Send + 'static,
          R: IntoFuture,
{
    type Item = R::Item;
    type Error = R::Error;

    fn poll(&mut self) -> Option<PollResult<R::Item, R::Error>> {
        match self.get() {
            Ok(f) => f.poll(),
            Err(e) => Some(Err(e)),
        }
    }

    fn await(&mut self) -> FutureResult<R::Item, R::Error> {
        try!(self.get()).await()
    }

    fn cancel(&mut self) {
        if let _Lazy::Second(ref mut f) = self.inner {
            return f.cancel()
        }
        self.inner = _Lazy::Canceled;
    }

    fn schedule<G>(&mut self, g: G)
        where G: FnOnce(PollResult<R::Item, R::Error>) + Send + 'static
    {
        match self.get() {
            Ok(f) => f.schedule(g),
            Err(e) => g(Err(e)),
        }
    }

    fn schedule_boxed(&mut self, cb: Box<Callback<R::Item, R::Error>>) {
        // TODO: wut? UFCS?
        Future::schedule(self, |r| cb.call(r))
    }
}
