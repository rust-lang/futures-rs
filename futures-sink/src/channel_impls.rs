use {Async, Sink, Poll};
use futures_core::task;
use futures_channel::mpsc::{Sender, SendError, UnboundedSender};

/// The error type of `<Sender<T> as Sink>`
///
/// It will contain a value of type `T` if one was passed to `start_send`
/// after the channel was closed.
#[derive(Debug)]
pub struct ChannelClosed<T>(Option<T>);

impl<T> ChannelClosed<T> {
    /// Extract the inner message.
    pub fn into_inner(self) -> Option<T> {
        self.0
    }

    fn from_send_error(err: SendError<T>) -> Self {
        ChannelClosed(Some(err.into_inner()))
    }

    fn from_empty_send_error(_: SendError<()>) -> Self {
        ChannelClosed(None)
    }
}

impl<T> Sink for Sender<T> {
    type SinkItem = T;
    type SinkError = ChannelClosed<T>;

    fn poll_ready(&mut self, cx: &mut task::Context) -> Poll<(), Self::SinkError> {
        self.poll_ready(cx).map_err(ChannelClosed::from_empty_send_error)
    }

    fn start_send(&mut self, msg: T) -> Result<(), Self::SinkError> {
        self.start_send(msg).map_err(ChannelClosed::from_send_error)
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
        self.poll_ready(cx).map_err(ChannelClosed::from_empty_send_error)
    }

    fn start_send(&mut self, msg: T) -> Result<(), Self::SinkError> {
        self.start_send(msg).map_err(ChannelClosed::from_send_error)
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
    type SinkError = SendError<T>;

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
