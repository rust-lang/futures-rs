use std::mem;

use {Future, Poll, Executor};
use stream::{Stream, channel, Sender, Receiver, FutureSender};
use task;

/// A stream representing the completion of a spawned streaming subtask.
pub struct Spawn<S: Stream> {
    rx: Receiver<S::Item, S::Error>,
}

pub fn new<S, E>(stream: S, exec: E) -> Spawn<S>
    where S: Stream, E: Executor<SpawnWrap<S>>,
{
    let (tx, mut rx) = channel();
    exec.spawn(SpawnWrap { stream: stream, state: State::Running(tx) });
    Spawn { rx: rx }
}

impl<S: Stream> Stream for Spawn<S> {
    type Item = S::Item;
    type Error = S::Error;

    fn poll(&mut self) -> Poll<S::Item, S::Error> {
        match self.rx.poll() {
            Poll::Ok(Ok(x)) => Poll::Ok(x),
            Poll::Ok(Err(e)) => Poll::Err(e),
            Poll::NotReady => Poll::NotReady,

            // TODO: actually propagate the panic info
            Poll::Err(_) => panic!("Propagating panic from spawned subtask"),
        }
    }
}

/// An internal wrapper used by `spawn` to construct the parent future.
// Wraps a future with transmission along a oneshot, bailing out if the oneshot
// is cancelled.
pub struct SpawnWrap<S: Stream> {
    stream: S,
    state: State<S::Item, S::Error>,
}

enum State<T, E> {
    Running(Sender<T, E>),
    Sending(FutureSender<T, E>),
    Done,
}

impl<S: Stream> Future for SpawnWrap<S> {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Poll<(), ()> {
        match mem::replace(&mut self.state, State::Done) {
            State::Running(tx) => {
                let res = match self.stream.poll() {
                    Poll::NotReady => {
                        self.state = State::Running(tx);
                        return Poll::NotReady;
                    }
                    Poll::Ok(None) => {
                        return Poll::Ok(())
                    }
                    Poll::Ok(Some(v)) => Ok(v),
                    Poll::Err(e) => Err(e),
                };
                self.state = State::Sending(tx.send(res));
                task::yield_now();
                Poll::NotReady
            }
            State::Sending(f) => {
                match f.poll() {
                    Poll::NotReady => {
                        self.state = State::Sending(f);
                        return Poll::NotReady;
                    }
                    Poll::Err(_) => {
                        return Poll::Error(())
                    }
                    Poll::Ok(tx) => {
                        self.state = State::Running(tx);
                        task::yield_now();
                        Poll::NotReady
                    }
                }
            }
            State::Done => {
                Poll::Ok(())
            }
        }
    }
}
