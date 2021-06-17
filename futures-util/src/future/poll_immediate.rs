use super::assert_future;
use crate::FutureExt;
use core::pin::Pin;
use futures_core::task::{Context, Poll};
use futures_core::{FusedFuture, Future, Stream};

/// Future for the [`poll_immediate`](poll_immediate()) function.
#[derive(Debug, Clone)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct PollImmediate<T>(Option<T>);

impl<T, F> Future for PollImmediate<F>
where
    F: Future<Output = T>,
{
    type Output = Option<T>;

    #[inline]
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<T>> {
        // # Safety
        // This is the only time that this future will ever be polled.
        let inner =
            unsafe { self.get_unchecked_mut().0.take().expect("PollOnce polled after completion") };
        crate::pin_mut!(inner);
        match inner.poll(cx) {
            Poll::Ready(t) => Poll::Ready(Some(t)),
            Poll::Pending => Poll::Ready(None),
        }
    }
}

impl<T: Future> FusedFuture for PollImmediate<T> {
    fn is_terminated(&self) -> bool {
        self.0.is_none()
    }
}

/// Creates a stream that can be polled repeatedly until the future is done
/// ```
/// # futures::executor::block_on(async {
/// use futures::task::Poll;
/// use futures::{StreamExt, future, pin_mut};
/// use future::FusedFuture;
///
/// let f = async { 1_u32 };
/// pin_mut!(f);
/// let mut r = future::poll_immediate(f);
/// assert_eq!(r.next().await, Some(Poll::Ready(1)));
///
/// let f = async {futures::pending!(); 42_u8};
/// pin_mut!(f);
/// let mut p = future::poll_immediate(f);
/// assert_eq!(p.next().await, Some(Poll::Pending));
/// assert!(!p.is_terminated());
/// assert_eq!(p.next().await, Some(Poll::Ready(42)));
/// assert!(p.is_terminated());
/// assert_eq!(p.next().await, None);
/// # });
/// ```
impl<T, F> Stream for PollImmediate<F>
where
    F: Future<Output = T>,
{
    type Item = Poll<T>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        unsafe {
            // # Safety
            // We never move the inner value until it is done. We only get a reference to it.
            let inner = &mut self.get_unchecked_mut().0;
            let fut = match inner.as_mut() {
                // inner is gone, so we can signal that the stream is closed.
                None => return Poll::Ready(None),
                Some(inner) => inner,
            };
            let fut = Pin::new_unchecked(fut);
            Poll::Ready(Some(fut.poll(cx).map(|t| {
                // # Safety
                // The inner option value is done, so we need to drop it. We do it without moving it
                // by using drop in place. We then write over the value without trying to drop it first
                // This should uphold all the safety requirements of `Pin`
                std::ptr::drop_in_place(inner);
                std::ptr::write(inner, None);
                t
            })))
        }
    }
}

/// Creates a future that is immediately ready with an Option of a value.
///
/// # Examples
///
/// ```
/// # futures::executor::block_on(async {
/// use futures::future;
///
/// let r = future::poll_immediate(async { 1_u32 });
/// assert_eq!(r.await, Some(1));
///
/// let p = future::poll_immediate(future::pending::<i32>());
/// assert_eq!(p.await, None);
/// # });
/// ```
pub fn poll_immediate<F: Future>(f: F) -> PollImmediate<F> {
    assert_future::<Option<F::Output>, PollImmediate<F>>(PollImmediate(Some(f)))
}

/// Future for the [`poll_immediate_reuse`](poll_immediate_reuse()) function.
#[derive(Debug, Clone)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct PollImmediateReuse<T>(Option<T>);

impl<T, F> Future for PollImmediateReuse<F>
where
    F: Future<Output = T> + Unpin,
{
    type Output = Result<T, F>;

    #[inline]
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<T, F>> {
        let mut inner = self.get_mut().0.take().expect("PollOnceReuse polled after completion");
        match inner.poll_unpin(cx) {
            Poll::Ready(t) => Poll::Ready(Ok(t)),
            Poll::Pending => Poll::Ready(Err(inner)),
        }
    }
}

impl<T: Future + Unpin> FusedFuture for PollImmediateReuse<T> {
    fn is_terminated(&self) -> bool {
        self.0.is_none()
    }
}

/// Creates a stream that can be polled repeatedly until the future is done
/// ```
/// # futures::executor::block_on(async {
/// use futures::task::Poll;
/// use futures::{StreamExt, future};
/// use future::FusedFuture;
///
/// let mut r = future::poll_immediate_reuse(future::ready(1_u32));
/// assert_eq!(r.next().await, Some(Poll::Ready(1)));
///
/// let mut p = future::poll_immediate_reuse(Box::pin(async {futures::pending!(); 42_u8}));
/// assert_eq!(p.next().await, Some(Poll::Pending));
/// assert!(!p.is_terminated());
/// assert_eq!(p.next().await, Some(Poll::Ready(42)));
/// assert!(p.is_terminated());
/// assert_eq!(p.next().await, None);
/// # });
/// ```
impl<T, F> Stream for PollImmediateReuse<F>
where
    F: Future<Output = T> + Unpin,
{
    type Item = Poll<T>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let inner = &mut self.get_mut().0;
        let fut = match inner.as_mut() {
            // inner is gone, so we can signal that the stream is closed.
            None => return Poll::Ready(None),
            Some(inner) => inner,
        };
        let fut = Pin::new(fut);
        Poll::Ready(Some(fut.poll(cx).map(|t| {
            *inner = None;
            t
        })))
    }
}

/// Creates a future that is immediately ready with a Result of a value or the future.
///
/// # Examples
///
/// ```
/// # futures::executor::block_on(async {
/// use futures::future;
///
/// let r = future::poll_immediate_reuse(future::ready(1_i32));
/// assert_eq!(r.await.unwrap(), 1);
///
/// // futures::pending!() returns pending once and then evaluates to `()`
/// let p = future::poll_immediate_reuse(Box::pin(async {
///     futures::pending!();
///     42_u8
/// }));
/// match p.await {
///     Ok(_) => unreachable!(),
///     Err(e) => {
///         assert_eq!(e.await, 42);
///     }
/// }
/// # });
/// ```
pub fn poll_immediate_reuse<F: Future + Unpin>(f: F) -> PollImmediateReuse<F> {
    assert_future::<Result<F::Output, _>, PollImmediateReuse<F>>(PollImmediateReuse(Some(f)))
}
