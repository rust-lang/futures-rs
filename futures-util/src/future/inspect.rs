use futures_core::{Future, PollResult, Poll};
use futures_core::task;

/// Do something with the item of a future, passing it on.
///
/// This is created by the [`FutureExt::inspect`] method.
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

    fn poll(&mut self, cx: &mut task::Context) -> PollResult<A::Item, A::Error> {
        let res = self.future.poll(cx);
        if let Poll::Ready(Ok(ref e)) = res {
            (self.f.take().expect("cannot poll Inspect twice"))(&e);
        }
        res
    }
}
