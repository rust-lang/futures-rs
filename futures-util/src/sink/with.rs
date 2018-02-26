use core::mem;
use core::marker::PhantomData;

use futures_core::{IntoFuture, Future, Poll, Async, Stream};
use futures_core::task;
use futures_sink::{Sink};

/// Sink for the `Sink::with` combinator, chaining a computation to run *prior*
/// to pushing a value into the underlying sink.
#[derive(Debug)]
#[must_use = "sinks do nothing unless polled"]
pub struct With<S, U, Fut, F>
    where S: Sink,
          F: FnMut(U) -> Fut,
          Fut: IntoFuture,
{
    sink: S,
    f: F,
    state: State<Fut::Future, S::SinkItem>,
    _phantom: PhantomData<fn(U)>,
}

#[derive(Debug)]
enum State<Fut, T> {
    Empty,
    Process(Fut),
    Buffered(T),
}

impl<Fut, T> State<Fut, T> {
    fn is_empty(&self) -> bool {
        if let State::Empty = *self {
            true
        } else {
            false
        }
    }
}

pub fn new<S, U, Fut, F>(sink: S, f: F) -> With<S, U, Fut, F>
    where S: Sink,
          F: FnMut(U) -> Fut,
          Fut: IntoFuture<Item = S::SinkItem>,
          Fut::Error: From<S::SinkError>,
{
    With {
        state: State::Empty,
        sink: sink,
        f: f,
        _phantom: PhantomData,
    }
}

// Forwarding impl of Stream from the underlying sink
impl<S, U, Fut, F> Stream for With<S, U, Fut, F>
    where S: Stream + Sink,
          F: FnMut(U) -> Fut,
          Fut: IntoFuture
{
    type Item = S::Item;
    type Error = S::Error;

    fn poll_next(&mut self, cx: &mut task::Context) -> Poll<Option<S::Item>, S::Error> {
        self.sink.poll_next(cx)
    }
}

impl<S, U, Fut, F> With<S, U, Fut, F>
    where S: Sink,
          F: FnMut(U) -> Fut,
          Fut: IntoFuture<Item = S::SinkItem>,
          Fut::Error: From<S::SinkError>,
{
    /// Get a shared reference to the inner sink.
    pub fn get_ref(&self) -> &S {
        &self.sink
    }

    /// Get a mutable reference to the inner sink.
    pub fn get_mut(&mut self) -> &mut S {
        &mut self.sink
    }

    /// Consumes this combinator, returning the underlying sink.
    ///
    /// Note that this may discard intermediate state of this combinator, so
    /// care should be taken to avoid losing resources when this is called.
    pub fn into_inner(self) -> S {
        self.sink
    }

    fn poll(&mut self, cx: &mut task::Context) -> Poll<(), Fut::Error> {
        loop {
            match mem::replace(&mut self.state, State::Empty) {
                State::Empty => break,
                State::Process(mut fut) => {
                    match fut.poll(cx)? {
                        Async::Ready(item) => {
                            self.state = State::Buffered(item);
                        }
                        Async::Pending => {
                            self.state = State::Process(fut);
                            break
                        }
                    }
                }
                State::Buffered(item) => {
                    match self.sink.poll_ready(cx)? {
                        Async::Ready(()) => self.sink.start_send(item)?,
                        Async::Pending => {
                            self.state = State::Buffered(item);
                            break
                        }
                    }
                }
            }
        }

        if self.state.is_empty() {
            Ok(Async::Ready(()))
        } else {
            Ok(Async::Pending)
        }
    }
}

impl<S, U, Fut, F> Sink for With<S, U, Fut, F>
    where S: Sink,
          F: FnMut(U) -> Fut,
          Fut: IntoFuture<Item = S::SinkItem>,
          Fut::Error: From<S::SinkError>,
{
    type SinkItem = U;
    type SinkError = Fut::Error;

    fn poll_ready(&mut self, cx: &mut task::Context) -> Poll<(), Self::SinkError> {
        self.poll(cx)
    }

    fn start_send(&mut self, item: Self::SinkItem) -> Result<(), Self::SinkError> {
        self.state = State::Process((self.f)(item).into_future());
        Ok(())
    }

    fn poll_flush(&mut self, cx: &mut task::Context) -> Poll<(), Self::SinkError> {
        try_ready!(self.poll(cx));
        self.sink.poll_flush(cx).map_err(Into::into)
    }

    fn poll_close(&mut self, cx: &mut task::Context) -> Poll<(), Self::SinkError> {
        try_ready!(self.poll(cx));
        self.sink.poll_close(cx).map_err(Into::into)
    }
}
