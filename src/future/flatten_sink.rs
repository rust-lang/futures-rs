use std::mem;

use {Async, AsyncSink, StartSend, Poll, Future, Sink};


enum State<F: Future>
    where F::Item: Sink
{
    Future(F),
    Sink(F::Item),
    Error(<F::Item as Sink>::SinkError),
    Void,
}

/// Future for the `into_sink` combinator, flattening a future-of-a-sink to
/// get just the result of the final sink instead
///
/// This is created by the `Future::into_sink` method.
pub struct FlattenSink<F: Future>
    where F::Item: Sink
{
    state: State<F>,
}

pub fn new<F>(future: F) -> FlattenSink<F>
    where F: Future,
          F::Item: Sink
{
    FlattenSink {
        state: State::Future(future),
    }
}

impl<F: Future> Sink for FlattenSink<F>
    where F::Item: Sink,
          <F::Item as Sink>::SinkError: From<F::Error>,
{
    type SinkItem = <F::Item as Sink>::SinkItem;
    type SinkError = <F::Item as Sink>::SinkError;
    fn start_send(&mut self, item: Self::SinkItem)
        -> StartSend<Self::SinkItem, Self::SinkError>
    {
        let (res, state) = match mem::replace(&mut self.state, State::Void) {
            State::Future(mut fut) => {
                match fut.poll() {
                    Ok(Async::Ready(mut sink)) => {
                        (sink.start_send(item)?, State::Sink(sink))
                    }
                    Ok(Async::NotReady) => {
                        (AsyncSink::NotReady(item), State::Future(fut))
                    }
                    Err(e) => {
                        (AsyncSink::NotReady(item), State::Error(e.into()))
                    }
                }
            }
            State::Sink(mut sink) => {
                (sink.start_send(item)?, State::Sink(sink))
            }
            State::Void => {
                panic!("Called FlattenSink::start_send after error");
            }
            State::Error(e) => (AsyncSink::NotReady(item), State::Error(e)),
        };
        self.state = state;
        return Ok(res);
    }
    fn poll_complete(&mut self) -> Poll<(), Self::SinkError> {
        let (res, state) = match mem::replace(&mut self.state, State::Void) {
            State::Future(mut fut) => {
                match fut.poll()? {
                    Async::Ready(sink) => {
                        (Async::Ready(()), State::Sink(sink))
                    }
                    Async::NotReady => {
                        (Async::NotReady, State::Future(fut))
                    }
                }
            }
            State::Sink(mut sink) => {
                (sink.poll_complete()?, State::Sink(sink))
            }
            State::Void => {
                panic!("Called FlattenSink::start_send after error");
            }
            State::Error(e) => return Err(e),
        };
        self.state = state;
        return Ok(res);
    }
}
