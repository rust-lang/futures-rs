use future::{Future, IntoFuture, ReadyQueue};
use stream::Stream;
use poll::Poll;

use std::prelude::v1::*;

/// An adaptor for a stream of futures to execute the futures concurrently, if
/// possible, delivering results as they become available.
///
/// This adaptor will return their results in the order that they complete.
/// This is created by the `futures` method.
///
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct FuturesUnordered<F>
    where F: Future
{
    queue: ReadyQueue<F>,
}

/// Converts a list of futures into a `Stream` of results from the futures.
///
/// This function will take an list of futures (e.g. a vector, an iterator,
/// etc), and return a stream. The stream will yield items as they become
/// available on the futures internally, in the order that they become
/// available. This function is similar to `buffer_unordered` in that it may
/// return items in a different order than in the list specified.
pub fn futures_unordered<I>(futures: I) -> FuturesUnordered<<I::Item as IntoFuture>::Future>
    where I: IntoIterator,
          I::Item: IntoFuture
{
    let mut queue = ReadyQueue::new();

    for future in futures {
        queue.push(future.into_future());
    }

    FuturesUnordered { queue: queue }
}

impl<F> Stream for FuturesUnordered<F>
    where F: Future
{
    type Item = F::Item;
    type Error = F::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        self.queue.poll()
    }
}
