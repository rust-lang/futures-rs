use {task, Future, Stream, Poll, Unpin};

#[cfg(feature = "either")]
use either::Either;

unsafe impl<T: Unpin> Unpin for Box<T> {}
unsafe impl<T: Unpin> Unpin for Option<T> {}
unsafe impl<'a, T: Unpin> Unpin for &'a T {}
unsafe impl<'a, T: Unpin> Unpin for &'a mut T {}

#[cfg(feature = "either")]
unsafe impl<T: Unpin, U: Unpin> Unpin for Either<T, U> {}

impl<'a, F: ?Sized + Future + Unpin> Future for &'a mut F {
    type Output = F::Output;

    fn poll_unpin(&mut self, cx: &mut task::Context) -> Poll<Self::Output>
        where &'a mut F: Unpin
    {
        (*self).poll_unpin(cx)
    }

    fn __must_impl_via_unpinned_macro() {}
}

impl<'a, S: ?Sized + Stream + Unpin> Stream for &'a mut S {
    type Item = S::Item;

    fn poll_next_mut(&mut self, cx: &mut task::Context) -> Poll<Option<Self::Item>> {
        (**self).poll_next_mut(cx)
    }

    fn __must_impl_via_unpinned_macro() {}
}

impl<T: Future + Unpin> Future for Option<T> {
    type Output = Option<T::Output>;

    fn poll_unpin(&mut self, cx: &mut task::Context) -> Poll<Self::Output> {
        let output = match *self {
            Some(ref mut fut) => {
                match fut.poll_unpin(cx) {
                    Poll::Ready(x) => Some(x),
                    Poll::Pending => return Poll::Pending,
                }
            }
            None => None
        };
        *self = None;
        Poll::Ready(output)
    }

    fn __must_impl_via_unpinned_macro() {}
}

if_std! {
    use std::boxed::Box;

    impl<'a, F: ?Sized + Future + Unpin> Future for Box<F> {
        type Output = F::Output;

        fn poll_unpin(&mut self, cx: &mut task::Context) -> Poll<Self::Output> {
            (**self).poll_unpin(cx)
        }

        fn __must_impl_via_unpinned_macro() {}
    }

    unsafe impl<T: Unpin> Unpin for ::std::panic::AssertUnwindSafe<T> {}
    impl<'a, F: Future + Unpin> Future for ::std::panic::AssertUnwindSafe<F> {
        type Output = F::Output;

        fn poll_unpin(&mut self, cx: &mut task::Context) -> Poll<Self::Output> {
            self.0.poll_unpin(cx)
        }

        fn __must_impl_via_unpinned_macro() {}
    }

    impl<S: ?Sized + Stream + Unpin> Stream for Box<S> {
        type Item = S::Item;

        fn poll_next_mut(&mut self, cx: &mut task::Context) -> Poll<Option<Self::Item>> {
            (**self).poll_next_mut(cx)
        }

        fn __must_impl_via_unpinned_macro() {}
    }

    impl<S: Stream + Unpin> Stream for ::std::panic::AssertUnwindSafe<S> {
        type Item = S::Item;

        fn poll_next_mut(&mut self, cx: &mut task::Context) -> Poll<Option<S::Item>> {
            self.0.poll_next_mut(cx)
        }

        fn __must_impl_via_unpinned_macro() {}
    }
}

#[cfg(feature = "either")]
impl<A, B> Future for Either<A, B>
    where A: Future + Unpin,
          B: Future<Output = A::Output> + Unpin,
{
    type Output = A::Output;

    fn poll_unpin(&mut self, cx: &mut task::Context) -> Poll<A::Output> {
        match *self {
            Either::Left(ref mut a) => a.poll_unpin(cx),
            Either::Right(ref mut b) => b.poll_unpin(cx),
        }
    }

    fn __must_impl_via_unpinned_macro() {}
}

#[cfg(feature = "either")]
impl<A, B> Stream for Either<A, B>
    where A: Stream + Unpin,
          B: Stream<Item = A::Item> + Unpin,
{
    type Item = A::Item;

    fn poll_next_mut(&mut self, cx: &mut task::Context) -> Poll<Option<A::Item>> {
        match *self {
            Either::Left(ref mut a) => a.poll_next_mut(cx),
            Either::Right(ref mut b) => b.poll_next_mut(cx),
        }
    }

    fn __must_impl_via_unpinned_macro() {}
}
