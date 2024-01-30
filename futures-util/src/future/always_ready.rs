use super::assert_future;
use core::pin::Pin;
use futures_core::future::{FusedFuture, Future};
use futures_core::task::{Context, Poll};

/// Future for the [`always_ready`](always_ready()) function.
#[derive(Debug, Clone, Copy)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct AlwaysReady<T, F: Fn() -> T>(F);

impl<T, F: Fn() -> T> Unpin for AlwaysReady<T, F> {}

impl<T, F: Fn() -> T> FusedFuture for AlwaysReady<T, F> {
    fn is_terminated(&self) -> bool {
        false
    }
}

impl<T, F: Fn() -> T> Future for AlwaysReady<T, F> {
    type Output = T;

    #[inline]
    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<T> {
        Poll::Ready(self.0())
    }
}

/// Creates a future that is always immediately ready with a value.
///
/// # Examples
///
/// ```
/// # futures::executor::block_on(async {
/// use std::mem::size_of_val;
///
/// use futures::future;
///
/// let a = future::always_ready(|| 1);
/// assert_eq!(size_of_val(&a), 0);
/// assert_eq!(a.await, 1);
/// assert_eq!(a.await, 1);
/// # });
/// ```
pub fn always_ready<'a, T, F>(prod: F) -> AlwaysReady<T, F>
where
    T: Copy + 'a,
    F: Fn() -> T + 'a,
{
    assert_future::<T, _>(AlwaysReady(prod))
}
