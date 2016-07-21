use std::mem;

use {Task, Future, Poll};
use stream::Stream;

/// A future used to collect all the results of a stream into one generic type.
///
/// This future is returned by the `Stream::fold` method.
pub struct Fold<S, F, Fut, T> {
    stream: S,
    f: F,
    state: State<T, Fut>,
}

enum State<T, Fut> {
    /// Placeholder state when doing work
    Empty,

    /// Ready to process the next stream item; current accumulator is the `T`
    Ready(T),

    /// Working on a future the process the previous stream item
    Processing(Fut),
}

pub fn new<S, F, Fut, T>(s: S, f: F, t: T) -> Fold<S, F, Fut, T>
    where S: Stream,
          F: FnMut(T, S::Item) -> Fut + Send + 'static,
          Fut: Future<Item = T>,
          Fut::Error: Into<S::Error>,
          T: Send + 'static
{
    Fold {
        stream: s,
        f: f,
        state: State::Ready(t),
    }
}

impl<S, F, Fut, T> Future for Fold<S, F, Fut, T>
    where S: Stream,
          F: FnMut(T, S::Item) -> Fut + Send + 'static,
          Fut: Future<Item = T>,
          Fut::Error: Into<S::Error>,
          T: Send + 'static
{
    type Item = T;
    type Error = S::Error;

    fn poll(&mut self, task: &mut Task) -> Poll<T, S::Error> {
        let mut task = task.scoped();
        loop {
            match mem::replace(&mut self.state, State::Empty) {
                State::Empty => panic!("cannot poll Fold twice"),
                State::Ready(state) => {
                    match self.stream.poll(&mut task) {
                        Poll::Ok(Some(e)) => {
                            self.state = State::Processing((self.f)(state, e))
                        }
                        Poll::Ok(None) => return Poll::Ok(state),
                        Poll::Err(e) => return Poll::Err(e),
                        Poll::NotReady => {
                            self.state = State::Ready(state);
                            return Poll::NotReady
                        }
                    }
                }
                State::Processing(mut fut) => {
                    match fut.poll(&mut task) {
                        Poll::Ok(state) => self.state = State::Ready(state),
                        Poll::Err(e) => return Poll::Err(e.into()),
                        Poll::NotReady => {
                            self.state = State::Processing(fut);
                            return Poll::NotReady;
                        }
                    }
                }
            }

            task.ready();
        }
    }

    fn schedule(&mut self, task: &mut Task) {
        match self.state {
            State::Empty => panic!("cannot `schedule` a completed Fold"),
            State::Ready(_) => self.stream.schedule(task),
            State::Processing(ref mut fut) => fut.schedule(task),
        }
    }
}
