use std::marker;
use std::sync::Arc;

use {Future, PollResult, Wake, Tokens};
use util;

/// A future representing a finished successful computation.
///
/// Created by the `finished` function.
pub struct Finished<T, E> {
    t: Option<T>,
    _e: marker::PhantomData<E>,
}

/// Creates a "leaf future" from an immediate value of a finished and
/// successful computation.
///
/// The returned future is similar to `done` where it will immediately run a
/// scheduled callback with the provided value.
///
/// # Examples
///
/// ```
/// use futures::*;
///
/// let future_of_1 = finished::<u32, u32>(1);
/// ```
pub fn finished<T, E>(t: T) -> Finished<T, E>
    where T: Send + 'static,
          E: Send + 'static,
{
    Finished { t: Some(t), _e: marker::PhantomData }
}

impl<T, E> Future for Finished<T, E>
    where T: Send + 'static,
          E: Send + 'static,
{
    type Item = T;
    type Error = E;


    fn poll(&mut self, _: &Tokens) -> Option<PollResult<T, E>> {
        Some(util::opt2poll(self.t.take()))
    }

    fn schedule(&mut self, wake: Arc<Wake>) -> Tokens {
        util::done(wake)
    }

    fn tailcall(&mut self) -> Option<Box<Future<Item=T, Error=E>>> {
        None
    }
}
