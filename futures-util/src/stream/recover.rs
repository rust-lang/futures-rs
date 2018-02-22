use core::marker::PhantomData;

use futures_core::{Stream, Poll, Async};
use futures_core::task;

/// Future for the `recover` combinator, handling errors by converting them into
/// an `Option<Item>`, such that a `None` value terminates the stream. `Recover`
/// is compatible with any error type of the caller's choosing.
#[must_use = "streams do nothing unless polled"]
#[derive(Debug)]
pub struct Recover<A, F, E> {
    inner: A,
    f: F,
    err: PhantomData<E>,
}

pub fn new<A, F, E>(stream: A, f: F) -> Recover<A, F, E>
    where A: Stream
{
    Recover { inner: stream, f: f, err: PhantomData }
}

impl<A, F, E> Stream for Recover<A, F, E>
    where A: Stream,
          F: FnMut(A::Error) -> Option<A::Item>,
{
    type Item = A::Item;
    type Error = E;

    fn poll_next(&mut self, cx: &mut task::Context) -> Poll<Option<A::Item>, E> {
        match self.inner.poll_next(cx) {
            Err(e) => Ok(Async::Ready((self.f)(e))),
            Ok(x) => Ok(x),
        }
    }
}
