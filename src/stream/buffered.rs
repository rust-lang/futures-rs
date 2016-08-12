use std::mem;

use {Task, IntoFuture, Poll, Future};
use stream::{Stream, Fuse};
use util::Collapsed;

/// An adaptor for a stream of futures to execute the futures concurrently, if
/// possible.
///
/// This adaptor will buffer up a list of pending futures, and then return their
/// results in the order that they were pulled out of the original stream. This
/// is created by the `Stream::buffered` method.
pub struct Buffered<S>
    where S: Stream,
          S::Item: IntoFuture,
{
    stream: Fuse<S>,
    futures: Vec<State<<S::Item as IntoFuture>::Future>>,
    cur: usize,
}

enum State<S: Future> {
    Empty,
    Running(Collapsed<S>),
    Finished(Result<S::Item, S::Error>),
}

pub fn new<S>(s: S, amt: usize) -> Buffered<S>
    where S: Stream,
          S::Item: IntoFuture<Error=<S as Stream>::Error>,
{
    Buffered {
        stream: super::fuse::new(s),
        futures: (0..amt).map(|_| State::Empty).collect(),
        cur: 0,
    }
}

impl<S> Stream for Buffered<S>
    where S: Stream,
          S::Item: IntoFuture<Error=<S as Stream>::Error>,
{
    type Item = <S::Item as IntoFuture>::Item;
    type Error = <S as Stream>::Error;

    fn poll(&mut self, task: &mut Task) -> Poll<Option<Self::Item>, Self::Error> {
        // First, try to fill in all the futures
        for i in 0..self.futures.len() {
            let mut idx = self.cur + i;
            if idx >= self.futures.len() {
                idx -= self.futures.len();
            }

            if let State::Empty = self.futures[idx] {
                match self.stream.poll(task) {
                    Poll::Ok(Some(future)) => {
                        let future = Collapsed::Start(future.into_future());
                        self.futures[idx] = State::Running(future);
                    }
                    Poll::Ok(None) => break,
                    Poll::Err(e) => return Poll::Err(e),
                    Poll::NotReady => break,
                }
            }
        }

        // Next, try and step all the futures forward
        for future in self.futures.iter_mut() {
            let result = match *future {
                State::Running(ref mut s) => {
                    match s.poll(task) {
                        Poll::Ok(e) => Ok(e),
                        Poll::Err(e) => Err(e),
                        Poll::NotReady => {
                            unsafe { s.collapse(); }
                            return Poll::NotReady
                        }
                    }
                }
                _ => continue,
            };
            *future = State::Finished(result);
        }

        // Check to see if our current future is done.
        if let State::Finished(_) = self.futures[self.cur] {
            let r = match mem::replace(&mut self.futures[self.cur], State::Empty) {
                State::Finished(r) => r,
                _ => panic!(),
            };
            self.cur += 1;
            if self.cur >= self.futures.len() {
                self.cur = 0;
            }
            return r.map(Some).into()
        }

        if self.stream.is_done() {
            if let State::Empty = self.futures[self.cur] {
                return Poll::Ok(None)
            }
        }
        Poll::NotReady
    }

    fn schedule(&mut self, task: &mut Task) {
        // If we've got an empty slot, then we're immediately ready to go.
        for slot in self.futures.iter() {
            if let State::Empty = *slot {
                return task.notify()
            }
        }

        // If the current slot is ready, we're ready to go
        if let State::Finished(_) = self.futures[self.cur] {
            return task.notify()
        }

        for slot in self.futures.iter_mut() {
            if let State::Running(ref mut s) = *slot {
                s.schedule(task);
            }
        }
    }
}
