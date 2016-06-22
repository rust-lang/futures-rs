use std::mem;

use {Future, PollResult, Callback, IntoFuture};
use executor::{Executor, DEFAULT};
use util;

/// A future which defers creation of the actual future until a callback is
/// scheduled.
///
/// This is created by the `lazy` function.
pub struct Lazy<F, R> {
    inner: _Lazy<F, R>
}

enum _Lazy<F, R> {
    First(F),
    Second(R),
    Moved,
}

/// Creates a new future which will eventually be the same as the one created
/// by the closure provided.
///
/// The provided closure is only run once the future has a callback scheduled
/// on it, otherwise the callback never runs. Once run, however, this future is
/// the same as the one the closure creates.
///
/// # Examples
///
/// ```
/// use futures::*;
///
/// let a = lazy(|| finished::<u32, u32>(1));
///
/// let b = lazy(|| -> Done<u32, u32> {
///     panic!("oh no!")
/// });
/// drop(b); // closure is never run
/// ```
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
