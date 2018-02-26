use {task, Future, Poll, Stream};

use either::Either;

impl<A, B> Future for Either<A, B>
    where A: Future,
          B: Future<Item = A::Item, Error = A::Error>
{
    type Item = A::Item;
    type Error = A::Error;

    fn poll(&mut self, cx: &mut task::Context) -> Poll<A::Item, A::Error> {
        match *self {
            Either::Left(ref mut a) => a.poll(cx),
            Either::Right(ref mut b) => b.poll(cx),
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
            Either::Left(ref mut a) => a.poll_next(cx),
            Either::Right(ref mut b) => b.poll_next(cx),
        }
    }
}
