use futures_core::{Future, Poll, Stream};
use futures_core::task;

/// A `Future` which is `Either` future `A` or future `B`.
/// 
/// This type can be used to return either one of two different futures
/// from the same function, provided that the futures have the same
/// `Item` and `Error` types.
#[derive(Debug)]
pub enum Either<A, B> {
    /// First branch of the type
    A(A),
    /// Second branch of the type
    B(B),
}

impl<T, A, B> Either<(T, A), (T, B)> {
    /// Splits out the homogeneous type from an either of tuples.
    ///
    /// This method is typically useful when combined with the `Future::select2`
    /// combinator.
    pub fn split(self) -> (T, Either<A, B>) {
        match self {
            Either::A((a, b)) => (a, Either::A(b)),
            Either::B((a, b)) => (a, Either::B(b)),
        }
    }
}

impl<A, B> Future for Either<A, B>
    where A: Future,
          B: Future<Item = A::Item, Error = A::Error>
{
    type Item = A::Item;
    type Error = A::Error;

    fn poll(&mut self, cx: &mut task::Context) -> Poll<A::Item, A::Error> {
        match *self {
            Either::A(ref mut a) => a.poll(cx),
            Either::B(ref mut b) => b.poll(cx),
        }
    }
}

impl<A, B> Stream for Either<A, B>
    where A: Stream,
          B: Stream<Item = A::Item, Error = A::Error>
{
    type Item = A::Item;
    type Error = A::Error;

    fn poll_next(&mut self, cx: &mut task::Context) -> Poll<Option<A::Item>, A::Error> {
        match *self {
            Either::A(ref mut a) => a.poll_next(cx),
            Either::B(ref mut b) => b.poll_next(cx),
        }
    }
}
