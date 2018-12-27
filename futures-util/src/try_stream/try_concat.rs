use core::default::Default;
use core::marker::Unpin;
use core::pin::Pin;
use futures_core::future::Future;
use futures_core::stream::TryStream;
use futures_core::task::{LocalWaker, Poll};
use pin_utils::{unsafe_pinned, unsafe_unpinned};

/// A stream combinator which attempts to concatenate the results of a stream into the
/// first yielded item.
///
/// This structure is produced by the `TryStream::try_concat` method.
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct TryConcat<St: TryStream> {
    stream: St,
    accum: Option<St::Ok>,
}

impl<St: TryStream + Unpin> Unpin for TryConcat<St> {}

impl<St> TryConcat<St>
where
    St: TryStream,
    St::Ok: Extend<<St::Ok as IntoIterator>::Item> + IntoIterator + Default,
{
    unsafe_pinned!(stream: St);
    unsafe_unpinned!(accum: Option<St::Ok>);

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

    fn poll(mut self: Pin<&mut Self>, lw: &LocalWaker) -> Poll<Self::Output> {
        loop {
            match ready!(self.as_mut().stream().try_poll_next(lw)) {
                Some(Ok(x)) => {
                    let accum = self.as_mut().accum();
                    if let Some(a) = accum {
                        a.extend(x)
                    } else {
                        *accum = Some(x)
                    }
                },
                Some(Err(e)) => return Poll::Ready(Err(e)),
                None => {
                    return Poll::Ready(Ok(self.as_mut().accum().take().unwrap_or_default()))
                }
            }
        }
    }
}
