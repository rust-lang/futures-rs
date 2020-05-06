use core::pin::Pin;
use futures_core::future::Future;
use futures_core::stream::TryStream;
use futures_core::task::{Context, Poll};
use pin_project::{pin_project, project};

/// Future for the [`try_concat`](super::TryStreamExt::try_concat) method.
#[pin_project]
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

    #[project]
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        #[project]
        let TryConcat { mut stream, accum } = self.project();
        Poll::Ready(Ok(loop {
            if let Some(x) = ready!(stream.as_mut().try_poll_next(cx)?) {
                if let Some(a) = accum {
                    a.extend(x)
                } else {
                    *accum = Some(x)
                }
            } else {
                break accum.take().unwrap_or_default();
            }
        }))
    }
}
