//! Definition of the FuturesOrdered combinator, executing each future in a sequence serially
//! and streaming their results.

use std::fmt;

use {Async, Future, IntoFuture, Poll, Stream};

/// A stream which takes a list of futures and executes them serially.
///
/// This future is created with the `futures_ordered` method.
#[must_use = "streams do nothing unless polled"]
pub struct FuturesOrdered<I>
    where I: IntoIterator,
          I::Item: IntoFuture
{
    elems: I::IntoIter,
    current: Option<<I::Item as IntoFuture>::Future>,
}

impl<I> fmt::Debug for FuturesOrdered<I>
    where I: IntoIterator,
          I::Item: IntoFuture,
          <<I as IntoIterator>::Item as IntoFuture>::Future: fmt::Debug,
{
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("FuturesOrdered")
            .field("current", &self.current)
            .finish()
    }
}

/// Converts a list of futures into a `Stream` of results from the futures,
/// executing each future serially.
///
/// This function will take a list of futures (e.g. a vector, an iterator,
/// etc), and return a stream. The stream will evaluate each future in order.
/// The next future in a list will not be evaluated until the current future
/// is complete.
///
/// A future returning an error will not make the stream invalid -- it will
/// move on to evaluating the next future in the list.
///
/// The stream concludes once all the futures in the list have been evaluated.
///
/// For a version of this that returns results as they become available, see
/// `futures_unordered`.
pub fn futures_ordered<I>(iter: I) -> FuturesOrdered<I>
    where I: IntoIterator,
          I::Item: IntoFuture
{
    let mut elems = iter.into_iter();
    let current = next_future(&mut elems);
    FuturesOrdered {
        elems: elems,
        current: current,
    }
}

impl<I> Stream for FuturesOrdered<I>
    where I: IntoIterator,
          I::Item: IntoFuture
{
    type Item = <I::Item as IntoFuture>::Item;
    type Error = <I::Item as IntoFuture>::Error;


    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        match self.current.take() {
            Some(mut fut) => {
                match fut.poll() {
                    Ok(Async::Ready(v)) => {
                        self.current = next_future(&mut self.elems);
                        Ok(Async::Ready(Some(v)))
                    }
                    Ok(Async::NotReady) => {
                        self.current = Some(fut);
                        Ok(Async::NotReady)
                    }
                    Err(e) => {
                        // Don't dump self.elems at this point because the
                        // caller might want to keep going on.
                        self.current = next_future(&mut self.elems);
                        Err(e)
                    }
                }
            }
            None => {
                // End of stream.
                Ok(Async::Ready(None))
            }
        }
    }
}

#[inline]
fn next_future<I>(elems: &mut I) -> Option<<I::Item as IntoFuture>::Future>
    where I: Iterator,
          I::Item: IntoFuture
{
    elems.next().map(IntoFuture::into_future)
}
