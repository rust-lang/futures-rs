use {Async, Sink, Poll};
use futures_core::task;
use futures_channel::mpsc::{Sender, ChannelClosed, UnboundedSender};

impl<T> Sink for Sender<T> {
    type SinkItem = T;
    type SinkError = ChannelClosed<T>;

    fn poll_ready(&mut self, cx: &mut task::Context) -> Poll<(), Self::SinkError> {
        self.poll_ready(cx)
    }

    fn start_send(&mut self, msg: T) -> Result<(), Self::SinkError> {
        self.start_send(msg)
    }

    fn start_close(&mut self) -> Result<(), Self::SinkError> {
        Ok(())
    }

    fn poll_flush(&mut self, _: &mut task::Context) -> Poll<(), Self::SinkError> {
        Ok(Async::Ready(()))
    }
}

impl<T> Sink for UnboundedSender<T> {
    type SinkItem = T;
    type SinkError = ChannelClosed<T>;

    fn poll_ready(&mut self, cx: &mut task::Context) -> Poll<(), Self::SinkError> {
        self.poll_ready(cx)
    }

    fn start_send(&mut self, msg: T) -> Result<(), Self::SinkError> {
        self.start_send(msg)
    }

    fn start_close(&mut self) -> Result<(), Self::SinkError> {
        Ok(())
    }

    fn poll_flush(&mut self, _: &mut task::Context) -> Poll<(), Self::SinkError> {
        Ok(Async::Ready(()))
    }
}

impl<'a, T> Sink for &'a UnboundedSender<T> {
    type SinkItem = T;
    type SinkError = ChannelClosed<T>;

    fn poll_ready(&mut self, _: &mut task::Context) -> Poll<(), Self::SinkError> {
        Ok(Async::Ready(()))
    }

    fn start_send(&mut self, msg: T) -> Result<(), Self::SinkError> {
        self.unbounded_send(msg)
    }

    fn start_close(&mut self) -> Result<(), Self::SinkError> {
        Ok(())
    }

    fn poll_flush(&mut self, _: &mut task::Context) -> Poll<(), Self::SinkError> {
        Ok(Async::Ready(()))
    }
}
