use future::{Future, IntoFuture};
use stream::Stream;
use poll::Poll;
use Async;
use stack::{Stack, Drain};
use std::sync::Arc;
use task::{self, UnparkEvent};

use std::prelude::v1::*;

/// An adaptor for a stream of futures to execute the futures concurrently, if
/// possible, delivering results as they become available.
///
/// This adaptor will return their results in the order that they complete.
/// This is created by the `futures` method.
///
#[must_use = "streams do nothing unless polled"]
pub struct FuturesUnordered<F>
    where F: Future
{
    futures: Vec<Option<F>>,
    stack: Arc<Stack<usize>>,
    pending: Option<Drain<usize>>,
    active: usize,
}

/// Converts a `Vec` of `IntoFuture` elements into a `Stream` over the underlying
/// futures. The created stream will yield into values produced by those futures.
/// Similar to `buffer_unordered` the order of the produced items will not be equal
/// with the order of elements in the input vector.
///
pub fn futures_unordered<I>(futures: I) -> FuturesUnordered<<I::Item as IntoFuture>::Future>
    where I: IntoIterator,
          I::Item: IntoFuture
{
    let futures = futures.into_iter().map(IntoFuture::into_future).map(Some).collect::<Vec<_>>();
    let stack = Arc::new(Stack::new());
    for i in 0..futures.len() {
        stack.push(i);
    }
    FuturesUnordered {
        active: futures.len(),
        futures: futures,
        pending: None,
        stack: stack,
    }
}

impl<F> FuturesUnordered<F>
    where F: Future
{
    fn poll_pending(&mut self, mut drain: Drain<usize>) -> Option<Poll<Option<F::Item>, F::Error>> {
        while let Some(id) = drain.next() {
            let event = UnparkEvent::new(self.stack.clone(), id);
            let ret = match task::with_unpark_event(event, || {
                self.futures[id]
                    .as_mut()
                    .unwrap()
                    .poll()
            }) {
                Ok(Async::NotReady) => {
                    continue;
                }
                Ok(Async::Ready(val)) => Ok(Async::Ready(Some(val))),
                Err(e) => Err(e),
            };
            self.pending = Some(drain);
            self.active -= 1;
            self.futures[id] = None;
            return Some(ret);
        }
        None
    }
}

impl<F> Stream for FuturesUnordered<F>
    where F: Future
{
    type Item = F::Item;
    type Error = F::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        if self.active == 0 {
            return Ok(Async::Ready(None));
        }
        let drain = self.pending
            .take()
            .unwrap_or_else(|| self.stack.drain());
        if let Some(ret) = self.poll_pending(drain){
            return ret;
        }
        let drain = self.stack.drain();
        if let Some(ret) = self.poll_pending(drain){
            return ret;
        }
        Ok(Async::NotReady)
    }
}
