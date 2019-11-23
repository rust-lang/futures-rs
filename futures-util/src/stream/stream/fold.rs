use core::fmt;
use core::pin::Pin;
use futures_core::future::{FusedFuture, Future};
use futures_core::iteration;
use futures_core::stream::Stream;
use futures_core::task::{Context, Poll};
use pin_utils::{unsafe_pinned, unsafe_unpinned};

/// Future for the [`fold`](super::StreamExt::fold) method.
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct Fold<St, Fut, T, F> {
    stream: St,
    f: F,
    accum: Option<T>,
    future: Option<Fut>,
    yield_after: iteration::Limit,
}

impl<St: Unpin, Fut: Unpin, T, F> Unpin for Fold<St, Fut, T, F> {}

impl<St, Fut, T, F> fmt::Debug for Fold<St, Fut, T, F>
where
    St: fmt::Debug,
    Fut: fmt::Debug,
    T: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Fold")
            .field("stream", &self.stream)
            .field("accum", &self.accum)
            .field("future", &self.future)
            .field("yield_after", &self.yield_after)
            .finish()
    }
}

impl<St, Fut, T, F> Fold<St, Fut, T, F>
where St: Stream,
      F: FnMut(T, St::Item) -> Fut,
      Fut: Future<Output = T>,
{
    unsafe_pinned!(stream: St);
    unsafe_unpinned!(f: F);
    unsafe_unpinned!(accum: Option<T>);
    unsafe_pinned!(future: Option<Fut>);
    unsafe_unpinned!(yield_after: iteration::Limit);

    fn split_borrows(
        self: Pin<&mut Self>,
    ) -> (
        Pin<&mut St>,
        &mut F,
        &mut Option<T>,
        Pin<&mut Option<Fut>>,
        &mut iteration::Limit,
    ) {
        unsafe {
            let this = self.get_unchecked_mut();
            (
                Pin::new_unchecked(&mut this.stream),
                &mut this.f,
                &mut this.accum,
                Pin::new_unchecked(&mut this.future),
                &mut this.yield_after,
            )
        }
    }

    future_method_yield_after_every! {
        #[pollee = "the underlying stream and, if pending, a future returned by
            the accumulation closure,"]
        #[why_busy = "the underlying stream consecutively yields items and
            the accumulation futures immediately resolve as ready,"]
    }

    pub(super) fn new(stream: St, f: F, t: T) -> Fold<St, Fut, T, F> {
        Fold {
            stream,
            f,
            accum: Some(t),
            future: None,
            yield_after: crate::DEFAULT_YIELD_AFTER_LIMIT,
       }
    }
}

impl<St, Fut, T, F> FusedFuture for Fold<St, Fut, T, F>
    where St: Stream,
          F: FnMut(T, St::Item) -> Fut,
          Fut: Future<Output = T>,
{
    fn is_terminated(&self) -> bool {
        self.accum.is_none() && self.future.is_none()
    }
}

impl<St, Fut, T, F> Future for Fold<St, Fut, T, F>
    where St: Stream,
          F: FnMut(T, St::Item) -> Fut,
          Fut: Future<Output = T>,
{
    type Output = T;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<T> {
        let (mut stream, op, accum, mut future, yield_after) = self.split_borrows();
        poll_loop! { yield_after, cx, {
            // we're currently processing a future to produce a new accum value
            if accum.is_none() {
                let acc = ready!(future.as_mut().as_pin_mut().unwrap().poll(cx));
                *accum = Some(acc);
                future.set(None);
            }

            let item = ready!(stream.as_mut().poll_next(cx));
            let accum = accum.take()
                .expect("Fold polled after completion");

            if let Some(e) = item {
                let fut = op(accum, e);
                future.as_mut().set(Some(fut));
            } else {
                return Poll::Ready(accum)
            }
        }}
    }
}
