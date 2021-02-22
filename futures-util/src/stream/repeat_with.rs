use super::assert_stream;
use crate::fns::FnMut0;
use core::pin::Pin;
use futures_core::stream::{Stream, FusedStream};
use futures_core::task::{Context, Poll};

/// An stream that repeats elements of type `A` endlessly by
/// applying the provided closure `F: FnMut() -> A`.
///
/// This `struct` is created by the [`repeat_with()`] function.
/// See its documentation for more.
#[derive(Debug, Clone)]
#[must_use = "streams do nothing unless polled"]
pub struct RepeatWith<F> {
    repeater: F,
}

impl<A, F: FnMut0<Output = A>> Unpin for RepeatWith<F> {}

impl<A, F: FnMut0<Output = A>> Stream for RepeatWith<F> {
    type Item = A;

    fn poll_next(mut self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Poll::Ready(Some(self.repeater.call_mut()))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (usize::max_value(), None)
    }
}

impl<A, F: FnMut0<Output = A>> FusedStream for RepeatWith<F> {
    fn is_terminated(&self) -> bool {
        false
    }
}

/// Creates a new stream that repeats elements of type `A` endlessly by
/// applying the provided closure, the repeater, `F: FnMut() -> A`.
///
/// The `repeat_with()` function calls the repeater over and over again.
///
/// Infinite stream like `repeat_with()` are often used with adapters like
/// [`stream.take()`], in order to make them finite.
///
/// If the element type of the stream you need implements [`Clone`], and
/// it is OK to keep the source element in memory, you should instead use
/// the [`stream.repeat()`] function.
///
/// # Examples
///
/// Basic usage:
///
/// ```
/// # futures::executor::block_on(async {
/// use futures::stream::{self, StreamExt};
///
/// // let's assume we have some value of a type that is not `Clone`
/// // or which don't want to have in memory just yet because it is expensive:
/// #[derive(PartialEq, Debug)]
/// struct Expensive;
///
/// // a particular value forever:
/// let mut things = stream::repeat_with(|| Expensive);
///
/// assert_eq!(Some(Expensive), things.next().await);
/// assert_eq!(Some(Expensive), things.next().await);
/// assert_eq!(Some(Expensive), things.next().await);
/// # });
/// ```
///
/// Using mutation and going finite:
///
/// ```rust
/// # futures::executor::block_on(async {
/// use futures::stream::{self, StreamExt};
///
/// // From the zeroth to the third power of two:
/// let mut curr = 1;
/// let mut pow2 = stream::repeat_with(|| { let tmp = curr; curr *= 2; tmp })
///                     .take(4);
///
/// assert_eq!(Some(1), pow2.next().await);
/// assert_eq!(Some(2), pow2.next().await);
/// assert_eq!(Some(4), pow2.next().await);
/// assert_eq!(Some(8), pow2.next().await);
///
/// // ... and now we're done
/// assert_eq!(None, pow2.next().await);
/// # });
/// ```
pub fn repeat_with<A, F: FnMut() -> A>(repeater: F) -> RepeatWith<F> {
    assert_stream::<A, _>(RepeatWith { repeater })
}

/// See [`repeat_with`].
///
/// # Examples
///
/// Basic usage:
///
/// ```
/// # futures::executor::block_on(async {
/// use futures_util::stream::{self, StreamExt};
///
/// // let's assume we have some value of a type that is not `Clone`
/// // or which don't want to have in memory just yet because it is expensive:
/// #[derive(PartialEq, Debug)]
/// struct Expensive;
///
/// // a particular value forever:
/// let mut things = stream::repeat_with_fns(|| Expensive);
///
/// assert_eq!(Some(Expensive), things.next().await);
/// assert_eq!(Some(Expensive), things.next().await);
/// assert_eq!(Some(Expensive), things.next().await);
/// # });
/// ```
///
/// Using mutation and going finite:
///
/// ```rust
/// # futures::executor::block_on(async {
/// use futures_util::stream::{self, StreamExt};
///
/// // From the zeroth to the third power of two:
/// let mut curr = 1;
/// let mut pow2 = stream::repeat_with_fns(|| { let tmp = curr; curr *= 2; tmp })
///                     .take(4);
///
/// assert_eq!(Some(1), pow2.next().await);
/// assert_eq!(Some(2), pow2.next().await);
/// assert_eq!(Some(4), pow2.next().await);
/// assert_eq!(Some(8), pow2.next().await);
///
/// // ... and now we're done
/// assert_eq!(None, pow2.next().await);
/// # });
/// ```
#[cfg(feature = "fntraits")]
#[cfg_attr(docsrs, doc(cfg(feature = "fntraits")))]
pub fn repeat_with_fns<A, F: FnMut0<Output = A>>(repeater: F) -> RepeatWith<F> {
    assert_stream::<A, _>(RepeatWith { repeater })
}
