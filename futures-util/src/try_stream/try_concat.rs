use core::pin::Pin;
use futures_core::future::Future;
use futures_core::stream::TryStream;
use futures_core::task::{Context, Poll};
use pin_project::{pin_project, unsafe_project};

/// Future for the [`try_concat`](super::TryStreamExt::try_concat) method.
#[unsafe_project(Unpin)]
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct TryConcat<St: TryStream> {
    #[pin]
    stream: St,
    accum: Option<St::Ok>,
}

impl<St> TryConcat<St>
where
    St: TryStream,
    St::Ok: Extend<<St::Ok as IntoIterator>::Item> + IntoIterator + Default,
{
    pub(super) fn new(stream: St) -> TryConcat<St> {
        TryConcat {
            stream,
            accum: None,
        }
    }
}

impl<St> Future for TryConcat<St>
where
    St: TryStream,
    St::Ok: Extend<<St::Ok as IntoIterator>::Item> + IntoIterator + Default,
{
    type Output = Result<St::Ok, St::Error>;

    #[pin_project(self)]
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        loop {
            match ready!(self.stream.as_mut().try_poll_next(cx)?) {
                Some(x) => {
                    if let Some(a) = self.accum {
                        a.extend(x)
                    } else {
                        *self.accum = Some(x)
                    }
                },
                None => {
                    return Poll::Ready(Ok(self.accum.take().unwrap_or_default()))
                }
            }
        }
    }
}
