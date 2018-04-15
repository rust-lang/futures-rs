use {Stream, Future, IntoFuture, Poll, Async};
use task;

/// A stream from a sequence of futures and blocks on each future step-by-step.
///
/// Created by the [`from_iter`](::stream::from_iter) function.
#[derive(Debug, Clone)]
pub struct StreamIterator<I, F> {
    iter: I,
    last: Option<F>,
}

impl<I, F> Stream for StreamIterator<I, F::Future>
    where I: Iterator<Item=F>,
          F: IntoFuture,
{
    type Item = <F::Future as Future>::Item;
    type Error = <F::Future as Future>::Error;

    fn poll_next(&mut self, cx: &mut task::Context) -> Poll<Option<Self::Item>, Self::Error> {
        match self.last {
            Some(ref mut fut) => Ok(fut.poll(cx)?.map(Some)),
            None => match self.iter.next() {
                Some(fut) => {
                    self.last = Some(fut.into_future());
                    Ok(self.last.as_mut().unwrap().poll(cx)?.map(Some))
                }
                None => Ok(Async::Ready(None)),
            }
        }
    }
}

/// Creates a new stream from a sequence of futures.
///
/// # Examples
///
/// ```
/// use futures_core::future;
/// use futures_core::stream::*;
///
/// let many_futures = vec![future::ok(1), future::err(2)];
/// let stream_from_futures = from_iter(many_futures);
/// ```
pub fn from_iter<I, F>(iter: I) -> StreamIterator<I::IntoIter, F::Future>
    where I: IntoIterator<Item=F>,
          F: IntoFuture,
{
    StreamIterator {
        iter: iter.into_iter(),
        last: None,
    }
}
