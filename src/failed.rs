use std::marker;

use {Future, Task, Poll};

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

    fn poll(&mut self, _: &mut Task) -> Poll<T, E> {
        Poll::Err(self.e.take().expect("cannot poll Failed twice"))
    }

    fn schedule(&mut self, task: &mut Task) {
        task.notify();
    }
}
