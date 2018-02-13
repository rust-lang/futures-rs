use futures_core::{Future, Poll, Async};
use futures_core::task;

/// Do something with the item of a future, passing it on.
///
/// This is created by the `Future::inspect` method.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct Inspect<A, F> where A: Future {
    future: A,
    f: Option<F>,
}

pub fn new<A, F>(future: A, f: F) -> Inspect<A, F>
    where A: Future,
          F: FnOnce(&A::Item),
{
    Inspect {
        future: future,
        f: Some(f),
    }
}

impl<A, F> Future for Inspect<A, F>
    where A: Future,
          F: FnOnce(&A::Item),
{
    type Item = A::Item;
    type Error = A::Error;

    fn poll(&mut self, cx: &mut task::Context) -> Poll<A::Item, A::Error> {
        match self.future.poll(cx) {
            Ok(Async::Pending) => Ok(Async::Pending),
            Ok(Async::Ready(e)) => {
                (self.f.take().expect("cannot poll Inspect twice"))(&e);
                Ok(Async::Ready(e))
            },
            Err(e) => Err(e),
        }
    }
}
