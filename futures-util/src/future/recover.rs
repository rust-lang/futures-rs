use core::marker::PhantomData;

use futures_core::{Future, Poll, Async};
use futures_core::task;

/// Future for the `recover` combinator, handling errors by converting them into
/// an `Item`, compatible with any error type of the caller's choosing.
#[must_use = "futures do nothing unless polled"]
#[derive(Debug)]
pub struct Recover<A, F, E> {
    inner: A,
    f: Option<F>,
    err: PhantomData<E>,
}

pub fn new<A, F, E>(future: A, f: F) -> Recover<A, F, E>
    where A: Future
{
    Recover { inner: future, f: Some(f), err: PhantomData }
}

impl<A, F, E> Future for Recover<A, F, E>
    where A: Future,
          F: FnOnce(A::Error) -> A::Item,
{
    type Item = A::Item;
    type Error = E;

    fn poll(&mut self, cx: &mut task::Context) -> Poll<A::Item, E> {
        match self.inner.poll(cx) {
            Err(e) => Ok(Async::Ready((self.f.take().expect("Polled future::Recover after completion"))(e))),
            Ok(x) => Ok(x),
        }
    }
}
