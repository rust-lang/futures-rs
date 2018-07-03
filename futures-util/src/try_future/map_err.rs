use core::mem::PinMut;

use futures_core::{Future, Poll, TryFuture};
use futures_core::task;

/// Future for the `map_err` combinator, changing the type of a future.
///
/// This is created by the `Future::map_err` method.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct MapErr<A, F> {
    future: A,
    f: Option<F>,
}

pub fn new<A, F>(future: A, f: F) -> MapErr<A, F> {
    MapErr {
        future,
        f: Some(f),
    }
}

impl<U, A, F> Future for MapErr<A, F>
    where A: TryFuture,
          F: FnOnce(A::Error) -> U,
{
    type Output = Result<A::Item, U>;

    fn poll(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<Self::Output> {
        match unsafe { pinned_field!(self, future) }.try_poll(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(e) => {
                let f = unsafe {
                    PinMut::get_mut_unchecked(self).f.take().expect("cannot poll MapErr twice")
                };
                Poll::Ready(e.map_err(f))
            }
        }
    }
}
