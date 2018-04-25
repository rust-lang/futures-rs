use futures_core::{Future, PollResult, Poll};
use futures_core::task;

/// Future for the `map` combinator, changing the type of a future.
///
/// This is created by the `Future::map` method.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct Map<A, F> where A: Future {
    future: A,
    f: Option<F>,
}

pub fn new<A, F>(future: A, f: F) -> Map<A, F>
    where A: Future,
{
    Map {
        future: future,
        f: Some(f),
    }
}

impl<U, A, F> Future for Map<A, F>
    where A: Future,
          F: FnOnce(A::Item) -> U,
{
    type Item = U;
    type Error = A::Error;

    fn poll(&mut self, cx: &mut task::Context) -> PollResult<U, A::Error> {
        let e = match self.future.poll(cx) {
            Poll::Pending => return Poll::Pending,
            Poll::Ready(r) => r,
        };
        Poll::Ready(e.map(self.f.take().expect("cannot poll Map twice")))
    }
}
