//! Asynchronous streams.

use core::ops::DerefMut;
use core::pin::Pin;
use core::task::{Context, Poll};

#[cfg(feature = "alloc")]
/// An owned dynamically typed [`Stream`] for use in cases where you can't
/// statically type your result or need to add some indirection.
pub type BoxStream<'a, T> = Pin<alloc::boxed::Box<dyn Stream<Item = T> + Send + 'a>>;

/// A stream of values produced asynchronously.
///
/// If `Future<Output = T>` is an asynchronous version of `T`, then `Stream<Item
/// = T>` is an asynchronous version of `Iterator<Item = T>`. A stream
/// represents a sequence of value-producing events that occur asynchronously to
/// the caller.
///
/// The trait is modeled after `Future`, but allows `poll_next` to be called
/// even after a value has been produced, yielding `None` once the stream has
/// been fully exhausted.
#[must_use = "streams do nothing unless polled"]
pub trait Stream {
    /// Values yielded by the stream.
    type Item;

    /// Attempt to pull out the next value of this stream, registering the
    /// current task for wakeup if the value is not yet available, and returning
    /// `None` if the stream is exhausted.
    ///
    /// # Return value
    ///
    /// There are several possible return values, each indicating a distinct
    /// stream state:
    ///
    /// - `Poll::Pending` means that this stream's next value is not ready
    /// yet. Implementations will ensure that the current task will be notified
    /// when the next value may be ready.
    ///
    /// - `Poll::Ready(Some(val))` means that the stream has successfully
    /// produced a value, `val`, and may produce further values on subsequent
    /// `poll_next` calls.
    ///
    /// - `Poll::Ready(None)` means that the stream has terminated, and
    /// `poll_next` should not be invoked again.
    ///
    /// # Panics
    ///
    /// Once a stream is finished, i.e. `Ready(None)` has been returned, further
    /// calls to `poll_next` may result in a panic or other "bad behavior".  If
    /// this is difficult to guard against then the `fuse` adapter can be used
    /// to ensure that `poll_next` always returns `Ready(None)` in subsequent
    /// calls.
    fn poll_next(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Self::Item>>;
}

impl<S: ?Sized + Stream + Unpin> Stream for &mut S {
    type Item = S::Item;

    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        S::poll_next(Pin::new(&mut **self), cx)
    }
}

impl<P> Stream for Pin<P>
where
    P: DerefMut + Unpin,
    P::Target: Stream,
{
    type Item = <P::Target as Stream>::Item;

    fn poll_next(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        self.get_mut().as_mut().poll_next(cx)
    }
}

/// A `Stream` or `TryStream` which tracks whether or not the underlying stream
/// should no longer be polled.
///
/// `is_terminated` will return `true` if a future should no longer be polled.
/// Usually, this state occurs after `poll_next` (or `try_poll_next`) returned
/// `Poll::Ready(None)`. However, `is_terminated` may also return `true` if a
/// stream has become inactive and can no longer make progress and should be
/// ignored or dropped rather than being polled again.
pub trait FusedStream {
    /// Returns `true` if the stream should no longer be polled.
    fn is_terminated(&self) -> bool;
}

impl<F: ?Sized + FusedStream> FusedStream for &mut F {
    fn is_terminated(&self) -> bool {
        <F as FusedStream>::is_terminated(&**self)
    }
}

impl<P> FusedStream for Pin<P>
where
    P: DerefMut + Unpin,
    P::Target: FusedStream,
{
    fn is_terminated(&self) -> bool {
        <P::Target as FusedStream>::is_terminated(&**self)
    }
}

/// A convenience for streams that return `Result` values that includes
/// a variety of adapters tailored to such futures.
pub trait TryStream {
    /// The type of successful values yielded by this future
    type Ok;

    /// The type of failures yielded by this future
    type Error;

    /// Poll this `TryStream` as if it were a `Stream`.
    ///
    /// This method is a stopgap for a compiler limitation that prevents us from
    /// directly inheriting from the `Stream` trait; in the future it won't be
    /// needed.
    fn try_poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>)
        -> Poll<Option<Result<Self::Ok, Self::Error>>>;
}

impl<S, T, E> TryStream for S
    where S: ?Sized + Stream<Item = Result<T, E>>
{
    type Ok = T;
    type Error = E;

    fn try_poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>)
        -> Poll<Option<Result<Self::Ok, Self::Error>>>
    {
        self.poll_next(cx)
    }
}

#[cfg(feature = "alloc")]
mod if_alloc {
    use alloc::boxed::Box;
    use super::*;

    impl<S: ?Sized + Stream + Unpin> Stream for Box<S> {
        type Item = S::Item;

        fn poll_next(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
        ) -> Poll<Option<Self::Item>> {
            Pin::new(&mut **self).poll_next(cx)
        }
    }

    #[cfg(feature = "std")]
    impl<S: Stream> Stream for ::std::panic::AssertUnwindSafe<S> {
        type Item = S::Item;

        fn poll_next(
            self: Pin<&mut Self>,
            cx: &mut Context<'_>,
        ) -> Poll<Option<S::Item>> {
            unsafe { Pin::map_unchecked_mut(self, |x| &mut x.0) }.poll_next(cx)
        }
    }

    impl<T: Unpin> Stream for ::alloc::collections::VecDeque<T> {
        type Item = T;

        fn poll_next(
            mut self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
        ) -> Poll<Option<Self::Item>> {
            Poll::Ready(self.pop_front())
        }
    }

    impl<S: ?Sized + FusedStream> FusedStream for Box<S> {
        fn is_terminated(&self) -> bool {
            <S as FusedStream>::is_terminated(&**self)
        }
    }
}
