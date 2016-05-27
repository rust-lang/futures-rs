use std::mem;

use {Future, PollResult, Callback, IntoFuture};
use executor::{Executor, DEFAULT};
use util;

pub struct Lazy<F, R> {
    inner: _Lazy<F, R>
}

enum _Lazy<F, R> {
    First(F),
    Second(R),
    Moved,
}

pub fn lazy<F, R>(f: F) -> Lazy<F, R::Future>
    where F: FnOnce() -> R + Send + 'static,
          R: IntoFuture
{
    Lazy { inner: _Lazy::First(f) }
}

impl<F, R> Future for Lazy<F, R::Future>
    where F: FnOnce() -> R + Send + 'static,
          R: IntoFuture,
{
    type Item = R::Item;
    type Error = R::Error;

    fn schedule<G>(&mut self, g: G)
        where G: FnOnce(PollResult<R::Item, R::Error>) + Send + 'static
    {
        match mem::replace(&mut self.inner, _Lazy::Moved) {
            _Lazy::First(f) => {
                let mut f = match util::recover(f) {
                    Ok(f) => f.into_future(),
                    Err(e) => return DEFAULT.execute(|| g(Err(e))),
                };
                f.schedule(g);
                self.inner = _Lazy::Second(f);
            }
            _Lazy::Second(f) => {
                self.inner = _Lazy::Second(f);
                DEFAULT.execute(|| g(Err(util::reused())))
            }
            _Lazy::Moved => {
                DEFAULT.execute(|| g(Err(util::reused())))
            }
        };
    }

    fn schedule_boxed(&mut self, cb: Box<Callback<R::Item, R::Error>>) {
        // TODO: wut? UFCS?
        Future::schedule(self, |r| cb.call(r))
    }
}
