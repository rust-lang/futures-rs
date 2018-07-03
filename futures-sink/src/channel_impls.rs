use crate::{Sink, Poll};
use futures_core::task;
use futures_channel::mpsc::{Sender, SendError, UnboundedSender};
use std::mem::PinMut;

impl<T> Sink for Sender<T> {
    type SinkItem = T;
    type SinkError = SendError;

    fn poll_ready(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<Result<(), Self::SinkError>> {
        (*self).poll_ready(cx)
    }

    fn start_send(mut self: PinMut<Self>, msg: T) -> Result<(), Self::SinkError> {
        (*self).start_send(msg)
    }

    fn poll_flush(self: PinMut<Self>, _: &mut task::Context) -> Poll<Result<(), Self::SinkError>> {
        Poll::Ready(Ok(()))
    }

    fn poll_close(mut self: PinMut<Self>, _: &mut task::Context) -> Poll<Result<(), Self::SinkError>> {
        self.close_channel();
        Poll::Ready(Ok(()))
    }
}

impl<T> Sink for UnboundedSender<T> {
    type SinkItem = T;
    type SinkError = SendError;

    fn poll_ready(self: PinMut<Self>, cx: &mut task::Context) -> Poll<Result<(), Self::SinkError>> {
        UnboundedSender::poll_ready(&*self, cx)
    }

    fn start_send(mut self: PinMut<Self>, msg: T) -> Result<(), Self::SinkError> {
        UnboundedSender::start_send(&mut *self, msg)
    }

    fn poll_flush(self: PinMut<Self>, _: &mut task::Context) -> Poll<Result<(), Self::SinkError>> {
        Poll::Ready(Ok(()))
    }

    fn poll_close(self: PinMut<Self>, _: &mut task::Context) -> Poll<Result<(), Self::SinkError>> {
        self.close_channel();
        Poll::Ready(Ok(()))
    }
}

impl<'a, T> Sink for &'a UnboundedSender<T> {
    type SinkItem = T;
    type SinkError = SendError;

    fn poll_ready(self: PinMut<Self>, cx: &mut task::Context) -> Poll<Result<(), Self::SinkError>> {
        UnboundedSender::poll_ready(*self, cx)
    }

    fn start_send(self: PinMut<Self>, msg: T) -> Result<(), Self::SinkError> {
        self.unbounded_send(msg)
            .map_err(|err| err.into_send_error())
    }

    fn poll_flush(self: PinMut<Self>, _: &mut task::Context) -> Poll<Result<(), Self::SinkError>> {
        Poll::Ready(Ok(()))
    }

    fn poll_close(self: PinMut<Self>, _: &mut task::Context) -> Poll<Result<(), Self::SinkError>> {
        self.close_channel();
        Poll::Ready(Ok(()))
    }
}
