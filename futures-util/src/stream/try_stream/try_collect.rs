use core::mem;
use core::pin::Pin;
use futures_core::future::{FusedFuture, Future};
use futures_core::stream::{FusedStream, TryStream};
use futures_core::task::{Context, Poll};
use pin_project::{pin_project, project};

/// Future for the [`try_collect`](super::TryStreamExt::try_collect) method.
#[pin_project]
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct TryCollect<St, C> {
    #[pin]
    stream: St,
    items: C,
}

impl<St: TryStream, C: Default> TryCollect<St, C> {
    pub(super) fn new(s: St) -> TryCollect<St, C> {
        TryCollect {
            stream: s,
            items: Default::default(),
        }
    }
}

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

    #[project]
    fn poll(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Self::Output> {
        #[project]
        let TryCollect { mut stream, items } = self.project();
        Poll::Ready(Ok(loop {
            match ready!(stream.as_mut().try_poll_next(cx)?) {
                Some(x) => items.extend(Some(x)),
                None => break mem::replace(items, Default::default()),
            }
        }))
    }
}
