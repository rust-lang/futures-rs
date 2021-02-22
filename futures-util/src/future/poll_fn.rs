//! Definition of the `PollFn` adapter combinator

use super::assert_future;
use crate::fns::FnMut1;
use core::fmt;
use core::pin::Pin;
use futures_core::future::Future;
use futures_core::task::{Context, Poll};

/// Future for the [`poll_fn`] function.
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct PollFn<F> {
    f: F,
}

impl<F> Unpin for PollFn<F> {}

/// Creates a new future wrapping around a function returning [`Poll`].
///
/// Polling the returned future delegates to the wrapped function.
///
/// # Examples
///
/// ```
/// # futures::executor::block_on(async {
/// use futures::future::poll_fn;
/// use futures::task::{Context, Poll};
///
/// fn read_line(_cx: &mut Context<'_>) -> Poll<String> {
///     Poll::Ready("Hello, World!".into())
/// }
///
/// let read_future = poll_fn(read_line);
/// assert_eq!(read_future.await, "Hello, World!".to_owned());
/// # });
/// ```
pub fn poll_fn<T, F>(f: F) -> PollFn<F>
where
    F: FnMut(&mut Context<'_>) -> Poll<T>
{
    assert_future::<T, _>(PollFn { f })
}

/// See [`poll_fn`].
///
/// # Examples
///
/// ```
/// # futures::executor::block_on(async {
/// use futures_util::future::poll_fn_fns;
/// use futures::task::{Context, Poll};
///
/// fn read_line(_cx: &mut Context<'_>) -> Poll<String> {
///     Poll::Ready("Hello, World!".into())
/// }
///
/// let read_future = poll_fn_fns(read_line);
/// assert_eq!(read_future.await, "Hello, World!".to_owned());
/// # });
/// ```
#[cfg(feature = "fntraits")]
#[cfg_attr(docsrs, doc(cfg(feature = "fntraits")))]
#[allow(single_use_lifetimes)] // https://github.com/rust-lang/rust/issues/55058
pub fn poll_fn_fns<T, F>(f: F) -> PollFn<F>
where
    F: for<'a, 'b> FnMut1<&'a mut Context<'b>, Output = Poll<T>>
{
    assert_future::<T, _>(PollFn { f })
}

impl<F> fmt::Debug for PollFn<F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PollFn").finish()
    }
}

#[allow(single_use_lifetimes)] // https://github.com/rust-lang/rust/issues/55058
impl<T, F> Future for PollFn<F>
where
    F: for<'a, 'b> FnMut1<&'a mut Context<'b>, Output = Poll<T>>,
{
    type Output = T;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<T> {
        self.f.call_mut(cx)
    }
}
