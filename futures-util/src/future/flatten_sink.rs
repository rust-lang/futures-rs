use core::fmt;

use futures_core::{task, Async, Future};
use futures_sink::Sink;

#[derive(Debug)]
enum State<F> where F: Future, <F as Future>::Item: Sink {
    Waiting(F),
    Ready(F::Item),
    Closed,
}

/// Future for the `flatten_sink` combinator, flattening a
/// future-of-a-sink to get just the result of the final sink as a sink.
///
/// This is created by the `Future::flatten_sink` method.
pub struct FlattenSink<F> where F: Future, <F as Future>::Item: Sink {
    st: State<F>
}

impl<F> fmt::Debug for FlattenSink<F>
    where F: Future + fmt::Debug,
          <F as Future>::Item: Sink<SinkError=F::Error> + fmt::Debug,
{
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("FlattenStream")
            .field("state", &self.st)
            .finish()
    }
}

impl<F> Sink for FlattenSink<F> where F: Future, <F as Future>::Item: Sink<SinkError=F::Error> {
    type SinkItem = <<F as Future>::Item as Sink>::SinkItem;
    type SinkError = <<F as Future>::Item as Sink>::SinkError;

    fn poll_ready(&mut self, cx: &mut task::Context) -> Result<Async<()>, Self::SinkError> {
        let mut resolved_stream = match self.st {
            State::Ready(ref mut s) => return s.poll_ready(cx),
            State::Waiting(ref mut f) => match f.poll(cx)? {
                Async::Pending => return Ok(Async::Pending),
                Async::Ready(s) => s,
            },
            State::Closed => panic!("poll_ready called after eof"),
        };
        let result = resolved_stream.poll_ready(cx);
        self.st = State::Ready(resolved_stream);
        result
    }

    fn start_send(&mut self, item: Self::SinkItem) -> Result<(), Self::SinkError> {
        match self.st {
            State::Ready(ref mut s) => s.start_send(item),
            State::Waiting(_) => panic!("poll_ready not called first"),
            State::Closed => panic!("start_send called after eof"),
        }
    }

    fn poll_flush(&mut self, cx: &mut task::Context) -> Result<Async<()>, Self::SinkError> {
        match self.st {
            State::Ready(ref mut s) => s.poll_flush(cx),
            // if sink not yet resolved, nothing written ==> everything flushed
            State::Waiting(_) => Ok(Async::Ready(())),
            State::Closed => panic!("poll_flush called after eof"),
        }
    }

    fn poll_close(&mut self, cx: &mut task::Context) -> Result<Async<()>, Self::SinkError> {
        if let State::Ready(ref mut s) = self.st {
            try_ready!(s.poll_close(cx));
        }
        self.st = State::Closed;
        return Ok(Async::Ready(()));
    }
}

pub fn new<F>(fut: F) -> FlattenSink<F> where F: Future, <F as Future>::Item: Sink {
    FlattenSink {
        st: State::Waiting(fut)
    }
}
