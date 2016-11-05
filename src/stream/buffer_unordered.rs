use std::prelude::v1::*;
use std::sync::Arc;
use std::collections::VecDeque;

use slab::Slab;
use task::{self, EventSet, UnparkEvent};

use {Async, IntoFuture, Poll, Future};
use stream::{Stream, Fuse};
use stack::Stack;

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
    futures: Slab<<S::Item as IntoFuture>::Future>,
    cur: usize,
    stack: Arc<Stack<usize>>,
}

pub fn new<S>(s: S, amt: usize) -> BufferUnordered<S>
    where S: Stream,
          S::Item: IntoFuture<Error=<S as Stream>::Error>,
{
    BufferUnordered {
        stream: super::fuse::new(s),
        futures: Slab::with_capacity(amt),
        cur: 0,
        stack: Arc::new(Stack::new()),
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


impl<S> Stream for BufferUnordered<S>
    where S: Stream,
          S::Item: IntoFuture<Error=<S as Stream>::Error>,
{
    type Item = <S::Item as IntoFuture>::Item;
    type Error = <S as Stream>::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        // First, try to fill in all the futures
        while self.futures.len() < self.futures.capacity() {
            match try!(self.stream.poll()) {
                Async::Ready(Some(s)) => {
                    let idx = match self.futures.insert(s.into_future()) {
                        Ok(idx) => idx,
                        // Can't unwrap because future does not implement Debug
                        Err(_original_future) => unreachable!(),
                    };
                    self.stack.push(idx);
                }
                Async::Ready(None) => break,
                Async::NotReady => break,
            }
        }

        // Next, try and step futures forward until we find a ready one.
        // Always start at `cur` for fairness.
        let mut waiting = false;
        let mut drained = self.stack.drain();
        while let Some(idx) = drained.next() {
            let result = match self.futures.get_mut(idx) {
                Some(ref mut s) => {
                    let event = UnparkEvent::new(self.stack.clone(), idx);
                    let poll_result = task::with_unpark_event(event, || {
                        s.poll()
                    });
                    match poll_result {
                        Ok(Async::NotReady) => {
                            waiting = true;
                            continue
                        },
                        Ok(Async::Ready(e)) => Ok(Async::Ready(Some(e))),
                        Err(e) => Err(e),
                    }
                },
                None => continue,
            };
            self.futures.remove(idx);
            // push unresolved futures back
            for idx in drained {
                self.stack.push(idx);
            }
            return result;
        }

        Ok(if waiting || !self.stream.is_done() {
            Async::NotReady
        } else {
            Async::Ready(None)
        })
    }
}
