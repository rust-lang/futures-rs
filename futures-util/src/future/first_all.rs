use alloc::vec::Vec;
use core::iter::FromIterator;
use core::pin::Pin;
use futures_core::future::Future;
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
            Some(out) => Poll::Ready(out),
            None => Poll::Pending,
        }
    }
}

// We don't provide FusedFuture, because the overhead of implementing it (
// which requires clearing the vector after Ready is returned) is precisely
// the same as using .fuse()

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

#[test]
fn test_first_all() {
    use crate::task::noop_waker_ref;
    use futures_channel::oneshot::channel;

    let mut futures = vec![];
    let mut senders = vec![];

    for _ in 0..10 {
        let (send, recv) = channel();
        futures.push(recv);
        senders.push(send);
    }

    let (send, recv) = channel();
    futures.push(recv);

    for _ in 0..10 {
        let (send, recv) = channel();
        futures.push(recv);
        senders.push(send);
    }

    let mut fut = first_all(futures);
    let mut pinned = Pin::new(&mut fut);
    let mut context = Context::from_waker(noop_waker_ref());

    let poll = pinned.as_mut().poll(&mut context);
    assert_eq!(poll, Poll::Pending);

    send.send(10).unwrap();
    let poll = pinned.as_mut().poll(&mut context);
    assert_eq!(poll, Poll::Ready(Ok(10)));
}
