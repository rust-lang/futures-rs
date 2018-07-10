use core::marker::PhantomData;
use core::mem::PinMut;
use futures_core::future::{Future, TryFuture};
use futures_core::task::{self, Poll};

/// Future for the `err_into` combinator, changing the error type of a future.
///
/// This is created by the `Future::err_into` method.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct ErrInto<A, E> {
    future: A,
    _marker: PhantomData<E>,
}

impl<A, E> ErrInto<A, E> {
    unsafe_pinned!(future: A);
}

pub fn new<A, E>(future: A) -> ErrInto<A, E> {
    ErrInto {
        future,
        _marker: PhantomData,
    }
}

impl<A, E> Future for ErrInto<A, E>
    where A: TryFuture,
          A::Error: Into<E>,
{
    type Output = Result<A::Item, E>;

    fn poll(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<Self::Output> {
        match self.future().try_poll(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(e) => {
                Poll::Ready(e.map_err(Into::into))
            }
        }
    }
}
