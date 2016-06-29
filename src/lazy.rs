use std::any::Any;
use std::mem;
use std::sync::Arc;

use {Future, PollResult, Wake, IntoFuture, PollError, Tokens};
use util;

/// A future which defers creation of the actual future until a callback is
/// scheduled.
///
/// This is created by the `lazy` function.
pub struct Lazy<F, R> {
    inner: _Lazy<F, R>,
    // TODO: the handling of a panicked closure here is pretty bad, should
    //       refactor this or just delete this future, seems to have very little
    //       reason to exist any more.
    deferred_error: Option<Box<Any+Send>>,
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
    Lazy {
        inner: _Lazy::First(f),
        deferred_error: None,
    }
}

impl<F, R> Lazy<F, R::Future>
    where F: FnOnce() -> R + Send + 'static,
          R: IntoFuture,
{
    fn get<E>(&mut self) -> PollResult<&mut R::Future, E> {
        match self.inner {
            _Lazy::First(_) => {}
            _Lazy::Second(ref mut f) => return Ok(f),
            _Lazy::Moved => return Err(util::reused()),
        }
        let f = match mem::replace(&mut self.inner, _Lazy::Moved) {
            _Lazy::First(f) => try!(util::recover(f)),
            _ => panic!(),
        };
        self.inner = _Lazy::Second(f.into_future());
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

    fn poll(&mut self, tokens: &Tokens) -> Option<PollResult<R::Item, R::Error>> {
        if let Some(e) = self.deferred_error.take() {
            return Some(Err(PollError::Panicked(e)))
        }
        match self.get() {
            Ok(f) => f.poll(tokens),
            Err(e) => Some(Err(e)),
        }
    }

    fn schedule(&mut self, wake: Arc<Wake>) {
        if self.deferred_error.is_some() {
            return util::done(wake)
        }

        let err = match self.get::<()>() {
            Ok(f) => return f.schedule(wake),
            Err(PollError::Panicked(e)) => e,
            Err(_) => panic!(),
        };

        // TODO: put this in a better location?
        self.deferred_error = Some(err);
        util::done(wake)
    }

    fn tailcall(&mut self) -> Option<Box<Future<Item=R::Item, Error=R::Error>>> {
        if self.deferred_error.is_some() {
            return None
        }

        let err = match self.get::<()>() {
            Ok(f) => return f.tailcall(),
            Err(PollError::Panicked(e)) => e,
            Err(_) => panic!(),
        };

        // TODO: put this in a better location?
        self.deferred_error = Some(err);
        None
    }
}
