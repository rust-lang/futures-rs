use core::pin::Pin;
use futures_core::future::{Future, FusedFuture};
use futures_core::stream::{Stream, FusedStream};
use futures_core::task::{Context, Poll};
use pin_project::{pin_project, project};

/// Future for the [`concat`](super::StreamExt::concat) method.
#[pin_project]
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

    #[project]
    fn poll(
        self: Pin<&mut Self>, cx: &mut Context<'_>
    ) -> Poll<Self::Output> {
        #[project]
        let Concat { mut stream, accum } = self.project();

        loop {
            match ready!(stream.as_mut().poll_next(cx)) {
                None => {
                    return Poll::Ready(accum.take().unwrap_or_default())
                }
                Some(e) => {
                    if let Some(a) = accum {
                        a.extend(e)
                    } else {
                        *accum = Some(e)
                    }
                }
            }
        }
    }
}

impl<St> FusedFuture for Concat<St>
where St: FusedStream,
      St::Item: Extend<<St::Item as IntoIterator>::Item> +
                IntoIterator + Default,
{
    fn is_terminated(&self) -> bool {
        self.accum.is_none() && self.stream.is_terminated()
    }
}
