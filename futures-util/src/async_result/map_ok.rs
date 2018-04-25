use core::mem::Pin;

use futures_core::{Async, Poll};
use futures_core::task;

use futures_core::AsyncResult;

/// Async for the `map_ok` combinator, changing the type of a future.
///
/// This is created by the `Async::map_ok` method.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct MapOk<A, F> {
    future: A,
    f: Option<F>,
}

pub fn new<A, F>(future: A, f: F) -> MapOk<A, F> {
    MapOk {
        future: future,
        f: Some(f),
    }
}

impl<U, A, F> Async for MapOk<A, F>
    where A: AsyncResult,
          F: FnOnce(A::Item) -> U,
{
    type Output = Result<U, A::Error>;

    fn poll(mut self: Pin<Self>, cx: &mut task::Context) -> Poll<Self::Output> {
        match unsafe { pinned_field!(self, future) }.poll(cx) {
            Poll::Pending => return Poll::Pending,
            Poll::Ready(e) => {
                let f = unsafe {
                    Pin::get_mut(&mut self).f.take().expect("cannot poll MapOk twice")
                };
                Poll::Ready(e.map(f))
            }
        }
    }
}
