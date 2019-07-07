use core::mem;
use core::pin::Pin;
use futures_core::future::{FusedFuture, Future};
use futures_core::stream::{FusedStream, TryStream};
use futures_core::task::{Context, Poll};
use pin_project::{pin_project, unsafe_project};

/// Future for the [`try_collect`](super::TryStreamExt::try_collect) method.
#[unsafe_project(Unpin)]
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

impl<St: FusedStream, C> FusedFuture for TryCollect<St, C> {
    fn is_terminated(&self) -> bool {
        self.stream.is_terminated()
    }
}

impl<St, C> Future for TryCollect<St, C>
    where St: TryStream, C: Default + Extend<St::Ok>
{
    type Output = Result<C, St::Error>;

    #[pin_project(self)]
    fn poll(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Self::Output> {
        loop {
            match ready!(self.stream.as_mut().try_poll_next(cx)?) {
                Some(x) => self.items.extend(Some(x)),
                None => return Poll::Ready(Ok(mem::replace(self.items, Default::default()))),
            }
        }
    }
}
