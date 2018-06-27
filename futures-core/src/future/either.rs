use {task, Stream, Poll};

use core::mem::PinMut;

use either::Either;

// impl<A, B> Future for Either<A, B>
//     where A: Future,
//           B: Future<Output = A::Output>
// {
//     type Output = A::Output;

//     fn poll(self: PinMut<Self>, cx: &mut task::Context) -> Poll<A::Output> {
//         unsafe {
//             match PinMut::get_mut(self) {
//                 Either::Left(a) => PinMut::new_unchecked(a).poll(cx),
//                 Either::Right(b) => PinMut::new_unchecked(b).poll(cx),
//             }
//         }
//     }
// }

impl<A, B> Stream for Either<A, B>
    where A: Stream,
          B: Stream<Item = A::Item>
{
    type Item = A::Item;

    fn poll_next(self: PinMut<Self>, cx: &mut task::Context) -> Poll<Option<A::Item>> {
        unsafe {
            match PinMut::get_mut_unchecked(self) {
                Either::Left(a) => PinMut::new_unchecked(a).poll_next(cx),
                Either::Right(b) => PinMut::new_unchecked(b).poll_next(cx),
            }
        }
    }
}
