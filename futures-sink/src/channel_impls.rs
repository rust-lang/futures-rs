use {Sink, Poll, PollResult};
use futures_core::task;
use futures_channel::mpsc::{Sender, SendError, UnboundedSender};

impl<T> Sink for Sender<T> {
    type SinkItem = T;
    type SinkError = SendError;

    fn poll_ready(&mut self, cx: &mut task::Context) -> PollResult<(), Self::SinkError> {
        self.poll_ready(cx)
    }

    fn start_send(&mut self, msg: T) -> Result<(), Self::SinkError> {
        self.start_send(msg)
    }

    fn poll_flush(&mut self, _: &mut task::Context) -> PollResult<(), Self::SinkError> {
        Poll::Ready(Ok(()))
    }

    fn poll_close(&mut self, _: &mut task::Context) -> PollResult<(), Self::SinkError> {
        self.close_channel();
        Poll::Ready(Ok(()))
    }
}

impl<T> Sink for UnboundedSender<T> {
    type SinkItem = T;
    type SinkError = SendError;

    fn poll_ready(&mut self, cx: &mut task::Context) -> PollResult<(), Self::SinkError> {
        UnboundedSender::poll_ready(&*self, cx)
    }

    fn start_send(&mut self, msg: T) -> Result<(), Self::SinkError> {
        self.start_send(msg)
    }

    fn poll_flush(&mut self, _: &mut task::Context) -> PollResult<(), Self::SinkError> {
        Poll::Ready(Ok(()))
    }

    fn poll_close(&mut self, _: &mut task::Context) -> PollResult<(), Self::SinkError> {
        self.close_channel();
        Poll::Ready(Ok(()))
    }
}

impl<'a, T> Sink for &'a UnboundedSender<T> {
    type SinkItem = T;
    type SinkError = SendError;

    fn poll_ready(&mut self, cx: &mut task::Context) -> PollResult<(), Self::SinkError> {
        UnboundedSender::poll_ready(*self, cx)
    }

    fn start_send(&mut self, msg: T) -> Result<(), Self::SinkError> {
        self.unbounded_send(msg)
            .map_err(|err| err.into_send_error())
    }

    fn poll_flush(&mut self, _: &mut task::Context) -> PollResult<(), Self::SinkError> {
        Poll::Ready(Ok(()))
    }

    fn poll_close(&mut self, _: &mut task::Context) -> PollResult<(), Self::SinkError> {
        self.close_channel();
        Poll::Ready(Ok(()))
    }
}
