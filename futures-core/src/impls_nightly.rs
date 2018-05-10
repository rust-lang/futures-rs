use {task, Future, Stream, Poll};
use core::mem::PinMut;

#[cfg(feature = "either")]
use either::Either;

impl<'a, F: ?Sized + Future> Future for &'a mut F {
    type Output = F::Output;

    fn poll(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<Self::Output> {
        unsafe { pinned_deref!(self).poll(cx) }
    }
}

impl<'a, S: ?Sized + Stream> Stream for &'a mut S {
    type Item = S::Item;

    fn poll_next(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<Option<Self::Item>> {
        unsafe { pinned_deref!(self).poll_next(cx) }
    }
}

impl<T: Future> Future for Option<T> {
    type Output = Option<T::Output>;

    fn poll(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<Self::Output> {
        let output = match *unsafe { PinMut::get_mut(self.reborrow()) } {
            Some(ref mut fut) => {
                match unsafe { PinMut::new_unchecked(fut) }.poll(cx) {
                    Poll::Ready(x) => Some(x),
                    Poll::Pending => return Poll::Pending,
                }
            }
            None => None
        };
        unsafe { *PinMut::get_mut(self) = None };
        Poll::Ready(output)
    }
}

if_std! {
    use std::boxed::{Box, PinBox};

    impl<'a, F: ?Sized + Future> Future for Box<F> {
        type Output = F::Output;

        fn poll(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<Self::Output> {
            unsafe { pinned_deref!(self).poll(cx) }
        }
    }

    impl<'a, F: ?Sized + Future> Future for PinBox<F> {
        type Output = F::Output;

        fn poll(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<Self::Output> {
            self.as_pin_mut().poll(cx)
        }
    }

    impl<'a, F: Future> Future for ::std::panic::AssertUnwindSafe<F> {
        type Output = F::Output;

        fn poll(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<Self::Output> {
            unsafe { pinned_field!(self, 0).poll(cx) }
        }
    }


    impl<S: ?Sized + Stream> Stream for Box<S> {
        type Item = S::Item;

        fn poll_next(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<Option<Self::Item>> {
            unsafe { pinned_deref!(self).poll_next(cx) }
        }
    }

    impl<S: ?Sized + Stream> Stream for PinBox<S> {
        type Item = S::Item;

        fn poll_next(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<Option<Self::Item>> {
            self.as_pin_mut().poll_next(cx)
        }
    }

    impl<S: Stream> Stream for ::std::panic::AssertUnwindSafe<S> {
        type Item = S::Item;

        fn poll_next(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<Option<S::Item>> {
            unsafe { pinned_field!(self, 0).poll_next(cx) }
        }
    }
}

#[cfg(feature = "either")]
impl<A, B> Future for Either<A, B>
    where A: Future,
          B: Future<Output = A::Output>
{
    type Output = A::Output;

    fn poll(self: PinMut<Self>, cx: &mut task::Context) -> Poll<A::Output> {
        unsafe {
            match *(PinMut::get_mut(self)) {
                Either::Left(ref mut a) => PinMut::new_unchecked(a).poll(cx),
                Either::Right(ref mut b) => PinMut::new_unchecked(b).poll(cx),
            }
        }
    }
}

#[cfg(feature = "either")]
impl<A, B> Stream for Either<A, B>
    where A: Stream,
          B: Stream<Item = A::Item>
{
    type Item = A::Item;

    fn poll_next(self: PinMut<Self>, cx: &mut task::Context) -> Poll<Option<A::Item>> {
        unsafe {
            match *(PinMut::get_mut(self)) {
                Either::Left(ref mut a) => PinMut::new_unchecked(a).poll_next(cx),
                Either::Right(ref mut b) => PinMut::new_unchecked(b).poll_next(cx),
            }
        }
    }
}
