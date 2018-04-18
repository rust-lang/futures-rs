use {task, Future, Stream, Poll};

use core::mem::Pin;

use either::Either;

impl<A, B> Future for Either<A, B>
    where A: Future,
          B: Future<Output = A::Output>
{
    type Output = A::Output;

    fn poll(mut self: Pin<Self>, cx: &mut task::Context) -> Poll<A::Output> {
        unsafe {
            match *(Pin::get_mut(&mut self)) {
                Either::Left(ref mut a) => Pin::new_unchecked(a).poll(cx),
                Either::Right(ref mut b) => Pin::new_unchecked(b).poll(cx),
            }
        }
    }
}

impl<A, B> Stream for Either<A, B>
    where A: Stream,
          B: Stream<Item = A::Item>
{
    type Item = A::Item;

    fn poll_next(mut self: Pin<Self>, cx: &mut task::Context) -> Poll<Option<A::Item>> {
        unsafe {
            match *(Pin::get_mut(&mut self)) {
                Either::Left(ref mut a) => Pin::new_unchecked(a).poll_next(cx),
                Either::Right(ref mut b) => Pin::new_unchecked(b).poll_next(cx),
            }
        }
    }
}
