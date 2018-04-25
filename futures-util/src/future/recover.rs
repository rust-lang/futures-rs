use core::marker::PhantomData;

use futures_core::{Future, PollResult, Poll};
use futures_core::task;

/// Future for the `recover` combinator, handling errors by converting them into
/// an `Item`, compatible with any error type of the caller's choosing.
#[must_use = "futures do nothing unless polled"]
#[derive(Debug)]
pub struct Recover<A, E, F> {
    inner: A,
    f: Option<F>,
    err: PhantomData<E>,
}

pub fn new<A, E, F>(future: A, f: F) -> Recover<A, E, F>
    where A: Future
{
    Recover { inner: future, f: Some(f), err: PhantomData }
}

impl<A, E, F> Future for Recover<A, E, F>
    where A: Future,
          F: FnOnce(A::Error) -> A::Item,
{
    type Item = A::Item;
    type Error = E;

    fn poll(&mut self, cx: &mut task::Context) -> PollResult<A::Item, E> {
        match self.inner.poll(cx) {
            Poll::Ready(Err(e)) => Poll::Ready(Ok((self.f.take().expect("PollResulted future::Recover after completion"))(e))),
            Poll::Ready(Ok(x)) => Poll::Ready(Ok(x)),
            Poll::Pending => Poll::Pending,
        }
    }
}
