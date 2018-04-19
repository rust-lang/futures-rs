use core::mem::Pin;

use futures_core::{Future, Poll};
use futures_core::task;

use FutureResult;

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
        future: future,
        f: Some(f),
    }
}

impl<U, A, F> Future for MapErr<A, F>
    where A: FutureResult,
          F: FnOnce(A::Error) -> U,
{
    type Output = Result<A::Item, U>;

    fn poll(mut self: Pin<Self>, cx: &mut task::Context) -> Poll<Self::Output> {
        match unsafe { pinned_field!(self, future) }.poll_result(cx) {
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
