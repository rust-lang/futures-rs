//! Definition of the Empty combinator, a future that's never ready.

use core::marker;

use anchor_experiment::MovePinned;
use futures_core::{Future, FutureMove, Poll, Async};

/// A future which is never resolved.
///
/// This future can be created with the `empty` function.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct Empty<T, E> {
    _data: marker::PhantomData<(T, E)>,
}

unsafe impl<T, E> MovePinned for Empty<T, E> { }

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

    unsafe fn poll_unsafe(&mut self) -> Poll<T, E> {
        Ok(Async::Pending)
    }
}

impl<T, E> FutureMove for Empty<T, E> {
    fn poll_move(&mut self) -> Poll<T, E> {
        Ok(Async::Pending)
    }
}
