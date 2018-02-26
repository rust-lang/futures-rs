use core::mem;

use futures_core::{Future, Poll, IntoFuture, Async, Stream};
use futures_core::task;

/// A future used to collect all the results of a stream into one generic type.
///
/// This future is returned by the `Stream::fold` method.
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct Fold<S, Fut, T, F> where Fut: IntoFuture {
    stream: S,
    f: F,
    state: State<T, Fut::Future>,
}

#[derive(Debug)]
enum State<T, F> where F: Future {
    /// Placeholder state when doing work
    Empty,

    /// Ready to process the next stream item; current accumulator is the `T`
    Ready(T),

    /// Working on a future the process the previous stream item
    Processing(F),
}

pub fn new<S, Fut, T, F>(s: S, f: F, t: T) -> Fold<S, Fut, T, F>
    where S: Stream,
          F: FnMut(T, S::Item) -> Fut,
          Fut: IntoFuture<Item = T, Error = S::Error>,
{
    Fold {
        stream: s,
        f: f,
        state: State::Ready(t),
    }
}

impl<S, Fut, T, F> Future for Fold<S, Fut, T, F>
    where S: Stream,
          F: FnMut(T, S::Item) -> Fut,
          Fut: IntoFuture<Item = T, Error = S::Error>,
{
    type Item = T;
    type Error = S::Error;

    fn poll(&mut self, cx: &mut task::Context) -> Poll<T, S::Error> {
        loop {
            match mem::replace(&mut self.state, State::Empty) {
                State::Empty => panic!("cannot poll Fold twice"),
                State::Ready(state) => {
                    match self.stream.poll_next(cx)? {
                        Async::Ready(Some(e)) => {
                            let future = (self.f)(state, e);
                            let future = future.into_future();
                            self.state = State::Processing(future);
                        }
                        Async::Ready(None) => return Ok(Async::Ready(state)),
                        Async::Pending => {
                            self.state = State::Ready(state);
                            return Ok(Async::Pending)
                        }
                    }
                }
                State::Processing(mut fut) => {
                    match fut.poll(cx)? {
                        Async::Ready(state) => self.state = State::Ready(state),
                        Async::Pending => {
                            self.state = State::Processing(fut);
                            return Ok(Async::Pending)
                        }
                    }
                }
            }
        }
    }
}
