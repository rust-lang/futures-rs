use std::marker;

use {Future, PollResult, PollError, Callback};
use executor::{Executor, DEFAULT};
use util;

/// A future representing a finished but erroneous computation.
///
/// Created by the `failed` function.
pub struct Failed<T, E> {
    _t: marker::PhantomData<T>,
    e: Option<E>,
}

/// Creates a "leaf future" from an immediate value of a failed computation.
///
/// The returned future is similar to `done` where it will immediately run a
/// scheduled callback with the provided value.
///
/// # Examples
///
/// ```
/// use futures::*;
///
/// let future_of_err_1 = failed::<u32, u32>(1);
/// ```
pub fn failed<T, E>(e: E) -> Failed<T, E>
    where T: Send + 'static,
          E: Send + 'static,
{
    Failed { _t: marker::PhantomData, e: Some(e) }
}

impl<T, E> Future for Failed<T, E>
    where T: Send + 'static,
          E: Send + 'static,
{
    type Item = T;
    type Error = E;

    fn schedule<G>(&mut self, g: G)
        where G: FnOnce(PollResult<T, E>) + Send + 'static
    {
        let res = util::opt2poll(self.e.take())
                       .and_then(|e| Err(PollError::Other(e)));
        DEFAULT.execute(|| g(res))
    }

    fn schedule_boxed(&mut self, cb: Box<Callback<T, E>>) {
        self.schedule(|r| cb.call(r))
    }
}
