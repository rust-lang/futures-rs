use super::assert_future;
use core::pin::Pin;
use futures_core::future::Future;
use futures_core::task::{Context, Poll};

/// Future for the [`with_context`] function.
#[must_use = "futures do nothing unless you `.await` or poll them"]
#[derive(Debug)]
pub struct WithContext<F> {
    f: Option<F>,
}

impl<F> Unpin for WithContext<F> {}

/// Runs the closure `F` with the task [`Context`].
///
/// This can be used to, for example, poll futures by hand.
pub fn with_context<F, R>(f: F) -> WithContext<F>
where
    F: FnOnce(&mut Context<'_>) -> R,
{
    assert_future(WithContext { f: Some(f) })
}

impl<F, R> Future for WithContext<F>
where
    F: FnOnce(&mut Context<'_>) -> R,
{
    type Output = R;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Poll::Ready((self.f.take().expect("WithContext polled after completion"))(cx))
    }
}
