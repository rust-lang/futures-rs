use futures_core::{Future, Poll, Async};
use futures_core::task;

/// Do something with the error of a future, passing it on.
///
/// This is created by the [`FutureExt::inspect_err`] method.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct InspectErr<A, F> where A: Future {
    future: A,
    f: Option<F>,
}

pub fn new<A, F>(future: A, f: F) -> InspectErr<A, F>
    where A: Future,
          F: FnOnce(&A::Error),
{
    InspectErr {
        future: future,
        f: Some(f),
    }
}

impl<A, F> Future for InspectErr<A, F>
    where A: Future,
          F: FnOnce(&A::Error),
{
    type Item = A::Item;
    type Error = A::Error;

    fn poll(&mut self, cx: &mut task::Context) -> Poll<A::Item, A::Error> {
        match self.future.poll(cx) {
            Ok(Async::Pending) => Ok(Async::Pending),
            Ok(Async::Ready(e)) => Ok(Async::Ready(e)),
            Err(e) => {
                (self.f.take().expect("cannot poll InspectErr twice"))(&e);
                Err(e)
            },
        }
    }
}
