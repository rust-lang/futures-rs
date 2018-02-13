//! Definition of the Empty combinator, a future that's never ready.

use core::marker;

use futures_core::{Future, Poll, Async};
use futures_core::task;

/// A future which is never resolved.
///
/// This future can be created with the `empty` function.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct Empty<T, E> {
    _data: marker::PhantomData<(T, E)>,
}

/// Creates a future which never resolves, representing a computation that never
/// finishes.
///
/// The returned future will forever return `Async::Pending`.
pub fn empty<T, E>() -> Empty<T, E> {
    Empty { _data: marker::PhantomData }
}

impl<T, E> Future for Empty<T, E> {
    type Item = T;
    type Error = E;

    fn poll(&mut self, _: &mut task::Context) -> Poll<T, E> {
        Ok(Async::Pending)
    }
}
