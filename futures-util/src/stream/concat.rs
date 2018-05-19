use core::mem::PinMut;
use core::fmt::{Debug, Formatter, Result as FmtResult};
use core::default::Default;

use futures_core::{Future, Poll, Stream};
use futures_core::task;

/// A stream combinator to concatenate the results of a stream into the first
/// yielded item.
///
/// This structure is produced by the `Stream::concat` method.
#[must_use = "streams do nothing unless polled"]
pub struct Concat<S>
    where S: Stream,
{
    stream: S,
    accum: Option<S::Item>,
}

impl<S: Debug> Debug for Concat<S> where S: Stream, S::Item: Debug {
    fn fmt(&self, fmt: &mut Formatter) -> FmtResult {
        fmt.debug_struct("Concat")
            .field("accum", &self.accum)
            .finish()
    }
}

pub fn new<S>(s: S) -> Concat<S>
    where S: Stream,
          S::Item: Extend<<<S as Stream>::Item as IntoIterator>::Item> + IntoIterator + Default,
{
    Concat {
        stream: s,
        accum: None,
    }
}

// These methods are the only points of access going through `PinMut`
impl<S: Stream> Concat<S> {
    unsafe_pinned!(stream -> S);
    unsafe_unpinned!(accum -> Option<S::Item>);
}

impl<S> Future for Concat<S>
    where S: Stream,
          S::Item: Extend<<<S as Stream>::Item as IntoIterator>::Item> + IntoIterator + Default,

{
    type Output = S::Item;

    fn poll(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<Self::Output> {
        loop {
            match self.stream().poll_next(cx) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(None) => {
                    return Poll::Ready(self.accum().take().unwrap_or_else(|| Default::default()))
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
