use core::mem::PinMut;

use futures_core::{Future, Poll, TryFuture};
use futures_core::task;

/// Future for the `map_ok` combinator, changing the type of a future.
///
/// This is created by the `Future::map_ok` method.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct MapOk<A, F> {
    future: A,
    f: Option<F>,
}

impl<A, F> MapOk<A, F> {
    unsafe_pinned!(future -> A);
}

pub fn new<A, F>(future: A, f: F) -> MapOk<A, F> {
    MapOk {
        future,
        f: Some(f),
    }
}

impl<U, A, F> Future for MapOk<A, F>
    where A: TryFuture,
          F: FnOnce(A::Item) -> U,
{
    type Output = Result<U, A::Error>;

    fn poll(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<Self::Output> {
        match self.future().try_poll(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(e) => {
                let f = unsafe {
                    PinMut::get_mut_unchecked(self).f.take().expect("cannot poll MapOk twice")
                };
                Poll::Ready(e.map(f))
            }
        }
    }
}
