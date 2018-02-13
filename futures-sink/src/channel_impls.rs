use {Async, Sink, AsyncSink, StartSend, Poll};
use futures_core::task;
use futures_channel::mpsc::{Sender, SendError, UnboundedSender};

fn res_to_async_sink<T>(res: Result<(), T>) -> AsyncSink<T> {
    match res {
        Ok(()) => AsyncSink::Ready,
        Err(x) => AsyncSink::Pending(x),
    }
}

impl<T> Sink for Sender<T> {
    type SinkItem = T;
    type SinkError = SendError<T>;

    fn start_send(&mut self, ctx: &mut task::Context, msg: T) -> StartSend<T, SendError<T>> {
        self.start_send(ctx, msg).map(res_to_async_sink)
    }

    fn flush(&mut self, _: &mut task::Context) -> Poll<(), SendError<T>> {
        Ok(Async::Ready(()))
    }

    fn close(&mut self, _: &mut task::Context) -> Poll<(), SendError<T>> {
        Ok(Async::Ready(()))
    }
}

impl<T> Sink for UnboundedSender<T> {
    type SinkItem = T;
    type SinkError = SendError<T>;

    fn start_send(&mut self, ctx: &mut task::Context, msg: T) -> StartSend<T, SendError<T>> {
        self.start_send(ctx, msg).map(res_to_async_sink)
    }

    fn flush(&mut self, _: &mut task::Context) -> Poll<(), SendError<T>> {
        Ok(Async::Ready(()))
    }

    fn close(&mut self, _: &mut task::Context) -> Poll<(), SendError<T>> {
        Ok(Async::Ready(()))
    }
}

impl<'a, T> Sink for &'a UnboundedSender<T> {
    type SinkItem = T;
    type SinkError = SendError<T>;

    fn start_send(&mut self, _: &mut task::Context, msg: T) -> StartSend<T, SendError<T>> {
        self.unbounded_send(msg)?;
        Ok(AsyncSink::Ready)
    }

    fn flush(&mut self, _: &mut task::Context) -> Poll<(), SendError<T>> {
        Ok(Async::Ready(()))
    }

    fn close(&mut self, _: &mut task::Context) -> Poll<(), SendError<T>> {
        Ok(Async::Ready(()))
    }
}
