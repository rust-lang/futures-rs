use core::pin::Pin;
use futures_core::future::Future;
use futures_core::iteration;
use futures_core::stream::TryStream;
use futures_core::task::{Context, Poll};
use pin_utils::{unsafe_pinned, unsafe_unpinned};

/// Future for the [`try_concat`](super::TryStreamExt::try_concat) method.
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct TryConcat<St: TryStream> {
    stream: St,
    accum: Option<St::Ok>,
    yield_after: iteration::Limit,
}

impl<St: TryStream + Unpin> Unpin for TryConcat<St> {}

impl<St> TryConcat<St>
where
    St: TryStream,
    St::Ok: Extend<<St::Ok as IntoIterator>::Item> + IntoIterator + Default,
{
    unsafe_pinned!(stream: St);
    unsafe_unpinned!(accum: Option<St::Ok>);
    unsafe_unpinned!(yield_after: iteration::Limit);

    fn split_borrows(
        self: Pin<&mut Self>,
    ) -> (Pin<&mut St>, &mut Option<St::Ok>, &mut iteration::Limit) {
        unsafe {
            let this = self.get_unchecked_mut();
            (
                Pin::new_unchecked(&mut this.stream),
                &mut this.accum,
                &mut this.yield_after,
            )
        }
    }

    try_future_method_yield_after_every!();

    pub(super) fn new(stream: St) -> TryConcat<St> {
        TryConcat {
            stream,
            accum: None,
            yield_after: crate::DEFAULT_YIELD_AFTER_LIMIT,
        }
    }
}

impl<St> Future for TryConcat<St>
where
    St: TryStream,
    St::Ok: Extend<<St::Ok as IntoIterator>::Item> + IntoIterator + Default,
{
    type Output = Result<St::Ok, St::Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let (mut stream, accum, yield_after) = self.split_borrows();
        poll_loop! { yield_after, cx,
            match ready!(stream.as_mut().try_poll_next(cx)?) {
                Some(x) => {
                    if let Some(a) = accum {
                        a.extend(x)
                    } else {
                        *accum = Some(x)
                    }
                },
                None => {
                    return Poll::Ready(Ok(accum.take().unwrap_or_default()))
                }
            }
        }
    }
}
