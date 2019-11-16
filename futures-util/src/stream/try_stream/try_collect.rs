use core::mem;
use core::pin::Pin;
use futures_core::future::{FusedFuture, Future};
use futures_core::iteration;
use futures_core::stream::{FusedStream, TryStream};
use futures_core::task::{Context, Poll};
use pin_utils::{unsafe_pinned, unsafe_unpinned};

/// Future for the [`try_collect`](super::TryStreamExt::try_collect) method.
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct TryCollect<St, C> {
    stream: St,
    items: C,
    yield_after: iteration::Limit,
}

impl<St: TryStream, C: Default> TryCollect<St, C> {
    unsafe_pinned!(stream: St);
    unsafe_unpinned!(items: C);
    unsafe_unpinned!(yield_after: iteration::Limit);

    fn split_borrows(self: Pin<&mut Self>) -> (Pin<&mut St>, &mut C, &mut iteration::Limit) {
        unsafe {
            let this = self.get_unchecked_mut();
            (
                Pin::new_unchecked(&mut this.stream),
                &mut this.items,
                &mut this.yield_after,
            )
        }
    }

    try_future_method_yield_after_every!();

    pub(super) fn new(s: St) -> TryCollect<St, C> {
        TryCollect {
            stream: s,
            items: Default::default(),
            yield_after: crate::DEFAULT_YIELD_AFTER_LIMIT,
        }
    }
}

impl<St: Unpin + TryStream, C> Unpin for TryCollect<St, C> {}

impl<St, C> FusedFuture for TryCollect<St, C>
where
    St: TryStream + FusedStream,
    C: Default + Extend<St::Ok>,
{
    fn is_terminated(&self) -> bool {
        self.stream.is_terminated()
    }
}

impl<St, C> Future for TryCollect<St, C>
where
    St: TryStream,
    C: Default + Extend<St::Ok>,
{
    type Output = Result<C, St::Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let (mut stream, items, yield_after) = self.split_borrows();
        poll_loop! { yield_after, cx,
            match ready!(stream.as_mut().try_poll_next(cx)?) {
                Some(x) => items.extend(Some(x)),
                None => return Poll::Ready(Ok(mem::replace(items, Default::default()))),
            }
        }
    }
}
