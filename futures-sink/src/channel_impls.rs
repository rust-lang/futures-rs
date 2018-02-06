use {Async, Sink, AsyncSink, StartSend, Poll};
use futures_channel::mpsc::{Sender, SendError, UnboundedSender};

impl<T> Sink for Sender<T> {
    type SinkItem = T;
    type SinkError = SendError<T>;

    fn start_send(&mut self, msg: T) -> StartSend<T, SendError<T>> {
        self.start_send(msg)
    }

    fn poll_complete(&mut self) -> Poll<(), SendError<T>> {
        Ok(Async::Ready(()))
    }

    fn close(&mut self) -> Poll<(), SendError<T>> {
        Ok(Async::Ready(()))
    }
}

impl<T> Sink for UnboundedSender<T> {
    type SinkItem = T;
    type SinkError = SendError<T>;

    fn start_send(&mut self, msg: T) -> StartSend<T, SendError<T>> {
        self.start_send(msg)
    }

    fn poll_complete(&mut self) -> Poll<(), SendError<T>> {
        Ok(Async::Ready(()))
    }

    fn close(&mut self) -> Poll<(), SendError<T>> {
        Ok(Async::Ready(()))
    }
}

impl<'a, T> Sink for &'a UnboundedSender<T> {
    type SinkItem = T;
    type SinkError = SendError<T>;

    fn start_send(&mut self, msg: T) -> StartSend<T, SendError<T>> {
        self.unbounded_send(msg)?;
        Ok(AsyncSink::Ready)
    }

    fn poll_complete(&mut self) -> Poll<(), SendError<T>> {
        Ok(Async::Ready(()))
    }

    fn close(&mut self) -> Poll<(), SendError<T>> {
        Ok(Async::Ready(()))
    }
}
