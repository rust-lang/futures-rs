use core::mem::Pin;

use futures_core::{Async, Poll};
use futures_core::task;

use futures_core::AsyncResult;

/// Async for the `map_err` combinator, changing the type of a future.
///
/// This is created by the `Async::map_err` method.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct MapErr<A, F> {
    future: A,
    f: Option<F>,
}

pub fn new<A, F>(future: A, f: F) -> MapErr<A, F> {
    MapErr {
        future: future,
        f: Some(f),
    }
}

impl<U, A, F> Async for MapErr<A, F>
    where A: AsyncResult,
          F: FnOnce(A::Error) -> U,
{
    type Output = Result<A::Item, U>;

    fn poll(mut self: Pin<Self>, cx: &mut task::Context) -> Poll<Self::Output> {
        match unsafe { pinned_field!(self, future) }.poll(cx) {
            Poll::Pending => return Poll::Pending,
            Poll::Ready(e) => {
                let f = unsafe {
                    Pin::get_mut(&mut self).f.take().expect("cannot poll MapErr twice")
                };
                Poll::Ready(e.map_err(f))
            }
        }
    }
}
