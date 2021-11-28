use crate::task::{noop_waker_ref, panic_waker_ref};
use futures_core::task::Context;

/// Create a new [`Context`](core::task::Context) where the
/// [waker](core::task::Context::waker) will panic if used.
///
/// # Examples
///
/// ```should_panic
/// use futures_test::task::panic_context;
///
/// let cx = panic_context();
/// cx.waker().wake_by_ref(); // Will panic
/// ```
pub fn panic_context() -> Context<'static> {
    Context::from_waker(panic_waker_ref())
}

/// Create a new [`Context`](core::task::Context) where the
/// [waker](core::task::Context::waker) will ignore any uses.
///
/// # Examples
///
/// ```
/// use futures::future::Future;
/// use futures::task::Poll;
/// use futures_test::task::noop_context;
/// use futures::pin_mut;
///
/// let future = async { 5 };
/// pin_mut!(future);
///
/// assert_eq!(future.poll(&mut noop_context()), Poll::Ready(5));
/// ```
pub fn noop_context() -> Context<'static> {
    Context::from_waker(noop_waker_ref())
}
