use core::iter::FromIterator;
use core::pin::Pin;
use futures_core::future::{FusedFuture, Future};
use futures_core::task::{Context, Poll};

/// Future for the [`first_all()`] function.
#[must_use = "futures do nothing unless you `.await` or poll them"]
#[derive(Debug, Clone)]
pub struct FirstAll<F> {
    // Critical safety invariant: after FirstAll is created, this vector can
    // never be reallocated, in order to ensure that Pin is upheld.
    futures: Vec<F>,
}

// Safety: once created, the contents of the vector don't change, and they'll
// remain in place permanently.
impl<F> Unpin for FirstAll<F> {}

impl<F: Future> Future for FirstAll<F> {
    type Output = F::Output;

    #[inline]
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        match this.futures.iter_mut().find_map(move |fut| {
            // Safety: we promise that the future is never moved out of the vec,
            // and that the vec never reallocates once FirstAll has been created
            // (specifically after the first poll)
            let pinned = unsafe { Pin::new_unchecked(fut) };
            match pinned.poll(cx) {
                Poll::Ready(out) => Some(out),
                Poll::Pending => None,
            }
        }) {
            Some(out) => {
                // Safety: safe because vec clears in place
                this.futures.clear();
                Poll::Ready(out)
            }
            None => Poll::Pending,
        }
    }
}

impl<F: FusedFuture> FusedFuture for FirstAll<F> {
    #[inline]
    fn is_terminated(&self) -> bool {
        // Logic: it's possible for a future to independently become
        // terminated, before it returns Ready, so we're not terminated unless
        // *all* of our inner futures are terminated. When our own poll returns
        // Ready, this vector is cleared, so the logic works correctly.
        self.futures.iter().all(|fut| fut.is_terminated())
    }
}

impl<Fut: Future> FromIterator<Fut> for FirstAll<Fut> {
    fn from_iter<T: IntoIterator<Item = Fut>>(iter: T) -> Self {
        first_all(iter)
    }
}

/// Creates a new future which will return the result of the first completed
/// future out of a list.
///
/// The returned future will wait for any future within `futures` to be ready.
/// Upon completion the item resolved will be returned.
///
/// The remaining futures will be discarded when the returned future is
/// dropped; see `select_all` for a version that returns the incomplete
/// futures if you need to poll over them further.
///
/// This function is only available when the `std` or `alloc` feature of this
/// library is activated, and it is activated by default.
///
/// # Panics
///
/// This function will panic if the iterator specified contains no items.
pub fn first_all<I>(futures: I) -> FirstAll<I::Item>
where
    I: IntoIterator,
    I::Item: Future,
{
    let futures = Vec::from_iter(futures);
    assert!(!futures.is_empty(), "Need at least 1 future for first_any");
    FirstAll { futures }
}
