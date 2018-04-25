use {task, Async, Poll};

use core::mem::Pin;
use either::Either;

impl<A, B> Async for Either<A, B>
    where A: Async,
          B: Async<Output = A::Output>
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
