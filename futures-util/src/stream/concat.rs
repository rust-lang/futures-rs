use core::fmt::{Debug, Formatter, Result as FmtResult};
use core::marker::Unpin;
use core::pin::Pin;
use core::default::Default;
use futures_core::future::Future;
use futures_core::stream::Stream;
use futures_core::task::{self, Poll};
use pin_utils::{unsafe_pinned, unsafe_unpinned};

/// A stream combinator to concatenate the results of a stream into the first
/// yielded item.
///
/// This structure is produced by the `Stream::concat` method.
#[must_use = "streams do nothing unless polled"]
pub struct Concat<St: Stream> {
    stream: St,
    accum: Option<St::Item>,
}

impl<St: Stream + Unpin> Unpin for Concat<St> {}

impl<St> Concat<St>
where St: Stream,
      St::Item: Extend<<St::Item as IntoIterator>::Item> +
                IntoIterator + Default,
{
    unsafe_pinned!(stream: St);
    unsafe_unpinned!(accum: Option<St::Item>);

    pub(super) fn new(stream: St) -> Concat<St> {
        Concat {
            stream,
            accum: None,
        }
    }
}

impl<St> Debug for Concat<St>
where St: Stream + Debug,
      St::Item: Debug,
{
    fn fmt(&self, fmt: &mut Formatter) -> FmtResult {
        fmt.debug_struct("Concat")
            .field("accum", &self.accum)
            .finish()
    }
}

impl<St> Future for Concat<St>
where St: Stream,
      St::Item: Extend<<St::Item as IntoIterator>::Item> +
                IntoIterator + Default,
{
    type Output = St::Item;

    fn poll(
        mut self: Pin<&mut Self>, cx: &mut task::Context
    ) -> Poll<Self::Output> {
        loop {
            match self.stream().poll_next(cx) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(None) => {
                    return Poll::Ready(self.accum().take().unwrap_or_default())
                }
                Poll::Ready(Some(e)) => {
                    let accum = self.accum();
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
