use std::prelude::v1::*;
use std::mem;
use std::sync::Arc;

use task::{self, UnparkEvent};

use {Async, IntoFuture, Poll, Future};
use stream::{Stream, Fuse};
use stack::{Stack, Drain};

/// An adaptor for a stream of futures to execute the futures concurrently, if
/// possible, delivering results as they become available.
///
/// This adaptor will buffer up a list of pending futures, and then return their
/// results in the order that they complete. This is created by the
/// `Stream::buffer_unordered` method.
#[must_use = "streams do nothing unless polled"]
pub struct BufferUnordered<S>
    where S: Stream,
          S::Item: IntoFuture,
{
    stream: Fuse<S>,
    futures: Vec<Slot<<S::Item as IntoFuture>::Future>>,
    next_future: usize,
    pending: Drain<usize>,
    stack: Arc<Stack<usize>>,
    active: usize,
}

enum Slot<T> {
    Next(usize),
    Data(T),
}

pub fn new<S>(s: S, amt: usize) -> BufferUnordered<S>
    where S: Stream,
          S::Item: IntoFuture<Error=<S as Stream>::Error>,
{
    let stack = Stack::new();
    BufferUnordered {
        stream: super::fuse::new(s),
        futures: (0..amt).map(|i| Slot::Next(i + 1)).collect(),
        next_future: 0,
        pending: stack.drain(),
        stack: Arc::new(stack),
        active: 0,
    }
}

impl<S> BufferUnordered<S>
    where S: Stream,
          S::Item: IntoFuture<Error=<S as Stream>::Error>,
{
    fn poll_pending(&mut self)
                    -> Option<Poll<Option<<S::Item as IntoFuture>::Item>,
                                   S::Error>> {
        while let Some(idx) = self.pending.next() {
            let result = match self.futures[idx] {
                Slot::Data(ref mut f) => {
                    let event = UnparkEvent::new(self.stack.clone(), idx);
                    match task::with_unpark_event(event, || f.poll()) {
                        Ok(Async::NotReady) => continue,
                        Ok(Async::Ready(e)) => Ok(Async::Ready(Some(e))),
                        Err(e) => Err(e),
                    }
                },
                Slot::Next(_) => continue,
            };
            self.active -= 1;
            self.futures[idx] = Slot::Next(self.next_future);
            self.next_future = idx;
            return Some(result)
        }
        None
    }
}

impl<S> Stream for BufferUnordered<S>
    where S: Stream,
          S::Item: IntoFuture<Error=<S as Stream>::Error>,
{
    type Item = <S::Item as IntoFuture>::Item;
    type Error = <S as Stream>::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        // First up, try to spawn off as many futures as possible by filling up
        // our slab of futures.
        while self.next_future < self.futures.len() {
            let future = match try!(self.stream.poll()) {
                Async::Ready(Some(s)) => s.into_future(),
                Async::Ready(None) |
                Async::NotReady => break,
            };
            self.active += 1;
            self.stack.push(self.next_future);
            match mem::replace(&mut self.futures[self.next_future],
                               Slot::Data(future)) {
                Slot::Next(next) => self.next_future = next,
                Slot::Data(_) => panic!(),
            }
        }

        // Next, see if our list of `pending` events from last time has any
        // items, and if so process them here.
        if let Some(ret) = self.poll_pending() {
            return ret
        }

        // And finally, take a look at our stack of events, attempting to
        // process all of those.
        assert!(self.pending.next().is_none());
        self.pending = self.stack.drain();
        if let Some(ret) = self.poll_pending() {
            return ret
        }

        // If we've gotten this far then there's no events for us to process and
        // nothing was ready, so figure out if we're not done yet or if we've
        // reached the end.
        Ok(if self.active > 0 || !self.stream.is_done() {
            Async::NotReady
        } else {
            Async::Ready(None)
        })
    }
}

// Forwarding impl of Sink from the underlying stream
impl<S> ::sink::Sink for BufferUnordered<S>
    where S: ::sink::Sink + Stream,
          S::Item: IntoFuture,
{
    type SinkItem = S::SinkItem;
    type SinkError = S::SinkError;

    fn start_send(&mut self, item: S::SinkItem) -> ::StartSend<S::SinkItem, S::SinkError> {
        self.stream.start_send(item)
    }

    fn poll_complete(&mut self) -> Poll<(), S::SinkError> {
        self.stream.poll_complete()
    }
}
