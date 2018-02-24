use futures_core::{Future, Poll, Async};
use futures_core::task;

use either::Either;

/// Future for the `select` combinator, waiting for one of two differently-typed
/// futures to complete.
///
/// This is created by the [`Future::select`] method.
///
/// [`Future::select`]: trait.Future.html#method.select
#[must_use = "futures do nothing unless polled"]
#[derive(Debug)]
pub struct Select<A, B> {
    inner: Option<(A, B)>,
}

pub fn new<A, B>(a: A, b: B) -> Select<A, B> {
    Select { inner: Some((a, b)) }
}

impl<A, B> Future for Select<A, B> where A: Future, B: Future {
    type Item = Either<(A::Item, B), (B::Item, A)>;
    type Error = Either<(A::Error, B), (B::Error, A)>;

    fn poll(&mut self, cx: &mut task::Context) -> Poll<Self::Item, Self::Error> {
        let (mut a, mut b) = self.inner.take().expect("cannot poll Select twice");
        match a.poll(cx) {
            Err(e) => Err(Either::Left((e, b))),
            Ok(Async::Ready(x)) => Ok(Async::Ready(Either::Left((x, b)))),
            Ok(Async::Pending) => match b.poll(cx) {
                Err(e) => Err(Either::Right((e, a))),
                Ok(Async::Ready(x)) => Ok(Async::Ready(Either::Right((x, a)))),
                Ok(Async::Pending) => {
                    self.inner = Some((a, b));
                    Ok(Async::Pending)
                }
            }
        }
    }
}
