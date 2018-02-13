use futures_core::{Future, Poll, Async};
use futures_core::task;

/// Future for the `map_err` combinator, changing the error type of a future.
///
/// This is created by the `Future::map_err` method.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct MapErr<A, F> where A: Future {
    future: A,
    f: Option<F>,
}

pub fn new<A, F>(future: A, f: F) -> MapErr<A, F>
    where A: Future
{
    MapErr {
        future: future,
        f: Some(f),
    }
}

impl<U, A, F> Future for MapErr<A, F>
    where A: Future,
          F: FnOnce(A::Error) -> U,
{
    type Item = A::Item;
    type Error = U;

    fn poll(&mut self, cx: &mut task::Context) -> Poll<A::Item, U> {
        let e = match self.future.poll(cx) {
            Ok(Async::Pending) => return Ok(Async::Pending),
            other => other,
        };
        e.map_err(self.f.take().expect("cannot poll MapErr twice"))
    }
}
