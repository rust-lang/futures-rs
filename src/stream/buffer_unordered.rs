use std::prelude::v1::*;

use {Async, IntoFuture, Poll, Future};
use stream::{Stream, Fuse};

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
    futures: Vec<Option<<S::Item as IntoFuture>::Future>>,
    cur: usize,
}

pub fn new<S>(s: S, amt: usize) -> BufferUnordered<S>
    where S: Stream,
          S::Item: IntoFuture<Error=<S as Stream>::Error>,
{
    BufferUnordered {
        stream: super::fuse::new(s),
        futures: (0..amt).map(|_| None).collect(),
        cur: 0,
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
        for future in &mut self.futures {
            if future.is_none() {
                match try!(self.stream.poll()) {
                    Async::Ready(Some(s)) => {
                        *future = Some(s.into_future());
                    }
                    Async::Ready(None) => break,
                    Async::NotReady => break,
                }
            }
        }

        // Next, try and step futures forward until we find a ready one.
        // Always start at `cur` for fairness.
        let mut waiting = false;
        for i in 0..self.futures.len() {
            let mut idx = self.cur + i;
            if idx >= self.futures.len() {
                idx -= self.futures.len();
            }
            let future = &mut self.futures[idx];
            let result = match *future {
                Some(ref mut s) => match s.poll() {
                    Ok(Async::NotReady) => {
                        waiting = true;
                        continue
                    },
                    Ok(Async::Ready(e)) => Ok(Async::Ready(Some(e))),
                    Err(e) => Err(e),
                },
                None => continue,
            };
            self.cur = i + 1;
            *future = None;
            return result;
        }

        Ok(if waiting || !self.stream.is_done() {
            Async::NotReady
        } else {
            Async::Ready(None)
        })
    }
}
