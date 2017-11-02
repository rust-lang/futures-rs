use {Future, Poll, IntoFuture, Async};
use stream::Stream;

use std::mem;

/// A stateful stream used to observe each item as it passes
///
/// This future is returned by the `Stream::scan` method.
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct Scanner<S, T, F, Fut> where Fut: IntoFuture {
    stream: S,
    scanner: T,
    future: State<Fut::Item,Fut::Future>,
    f: F,
}

#[derive(Debug)]
enum State<X,Fut> where Fut: Future {

    /// Uninitalized, either just yielded a value, or 
    Empty,

    /// Have an interior future to poll
    Future(Fut),

    /// Interior future yielded value
    Ready(X),
}

pub fn new<S, T, F, Fut>(s: S, t: T, f: F) -> Scanner<S, T, F, Fut>
    where S: Stream,
          Fut: IntoFuture<Error=S::Error>,
          F: FnMut(&mut T, S::Item) -> Fut
{
    Scanner {
        stream: s,
        scanner: t,
        future: State::Empty,
        f: f,
    }
}

impl<S, T, F, Fut> Stream for Scanner<S, T, F, Fut>
    where S: Stream,
          Fut: IntoFuture<Error=S::Error>,
          F: FnMut(&mut T, S::Item) -> Fut
{
    type Item = Fut::Item;
    type Error = S::Error;

    fn poll(&mut self) -> Poll<Option<Fut::Item>, S::Error> {
        loop {
            match mem::replace(&mut self.future, State::Empty) {
                State::Ready(x) => return Ok(Async::Ready(Some(x))),
                State::Empty => {
                    match self.stream.poll()? {
                        Async::NotReady => return Ok(Async::NotReady),
                        Async::Ready(None) => return Ok(Async::Ready(None)),
                        Async::Ready(Some(x)) => {
                            let future = (self.f)(&mut self.scanner, x);
                            let future = future.into_future();
                            self.future = State::Future(future);
                        }
                    }
                },
                State::Future(mut fut) => {
                    match fut.poll()? {
                        Async::Ready(x) => {
                            self.future = State::Ready(x);
                        }
                        Async::NotReady => {
                            self.future = State::Future(fut);
                            return Ok(Async::NotReady);
                        }
                    }
                }
            }
        }
    }
}

