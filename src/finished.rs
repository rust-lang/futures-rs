use core::marker;

use {Future, Poll, Async};

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
pub fn finished<T, E>(t: T) -> Finished<T, E> {
    Finished { t: Some(t), _e: marker::PhantomData }
}

impl<T, E> Future for Finished<T, E> {
    type Item = T;
    type Error = E;


    fn poll(&mut self) -> Poll<T, E> {
        Ok(Async::Ready(self.t.take().expect("cannot poll Finished twice")))
    }
}
