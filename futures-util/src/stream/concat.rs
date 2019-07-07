use core::pin::Pin;
use futures_core::future::Future;
use futures_core::stream::Stream;
use futures_core::task::{Context, Poll};
use pin_project::{pin_project, unsafe_project};

/// Future for the [`concat`](super::StreamExt::concat) method.
#[unsafe_project(Unpin)]
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct Concat<St: Stream> {
    #[pin]
    stream: St,
    accum: Option<St::Item>,
}

impl<St> Concat<St>
where St: Stream,
      St::Item: Extend<<St::Item as IntoIterator>::Item> +
                IntoIterator + Default,
{
    pub(super) fn new(stream: St) -> Concat<St> {
        Concat {
            stream,
            accum: None,
        }
    }
}

impl<St> Future for Concat<St>
where St: Stream,
      St::Item: Extend<<St::Item as IntoIterator>::Item> +
                IntoIterator + Default,
{
    type Output = St::Item;

    #[pin_project(self)]
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        loop {
            match ready!(self.stream.as_mut().poll_next(cx)) {
                None => {
                    return Poll::Ready(self.accum.take().unwrap_or_default())
                }
                Some(e) => {
                    if let Some(a) = self.accum {
                        a.extend(e)
                    } else {
                        *self.accum = Some(e)
                    }
                }
            }
        }
    }
}
