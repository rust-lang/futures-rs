use crate::{Sink, Poll};
use futures_core::task::Waker;
use futures_channel::mpsc::{Sender, SendError, TrySendError, UnboundedSender};
use std::pin::Pin;

impl<T> Sink<T> for Sender<T> {
    type SinkError = SendError;

    fn poll_ready(mut self: Pin<&mut Self>, waker: &Waker) -> Poll<Result<(), Self::SinkError>> {
        (*self).poll_ready(waker)
    }

    fn start_send(mut self: Pin<&mut Self>, msg: T) -> Result<(), Self::SinkError> {
        (*self).start_send(msg)
    }

    fn poll_flush(self: Pin<&mut Self>, _: &Waker) -> Poll<Result<(), Self::SinkError>> {
        Poll::Ready(Ok(()))
    }

    fn poll_close(mut self: Pin<&mut Self>, _: &Waker) -> Poll<Result<(), Self::SinkError>> {
        self.disconnect();
        Poll::Ready(Ok(()))
    }
}

impl<T> Sink<T> for UnboundedSender<T> {
    type SinkError = SendError;

    fn poll_ready(self: Pin<&mut Self>, waker: &Waker) -> Poll<Result<(), Self::SinkError>> {
        UnboundedSender::poll_ready(&*self, waker)
    }

    fn start_send(mut self: Pin<&mut Self>, msg: T) -> Result<(), Self::SinkError> {
        UnboundedSender::start_send(&mut *self, msg)
    }

    fn poll_flush(self: Pin<&mut Self>, _: &Waker) -> Poll<Result<(), Self::SinkError>> {
        Poll::Ready(Ok(()))
    }

    fn poll_close(mut self: Pin<&mut Self>, _: &Waker) -> Poll<Result<(), Self::SinkError>> {
        self.disconnect();
        Poll::Ready(Ok(()))
    }
}

impl<'a, T> Sink<T> for &'a UnboundedSender<T> {
    type SinkError = SendError;

    fn poll_ready(self: Pin<&mut Self>, waker: &Waker) -> Poll<Result<(), Self::SinkError>> {
        UnboundedSender::poll_ready(*self, waker)
    }

    fn start_send(self: Pin<&mut Self>, msg: T) -> Result<(), Self::SinkError> {
        self.unbounded_send(msg)
            .map_err(TrySendError::into_send_error)
    }

    fn poll_flush(self: Pin<&mut Self>, _: &Waker) -> Poll<Result<(), Self::SinkError>> {
        Poll::Ready(Ok(()))
    }

    fn poll_close(self: Pin<&mut Self>, _: &Waker) -> Poll<Result<(), Self::SinkError>> {
        self.close_channel();
        Poll::Ready(Ok(()))
    }
}
