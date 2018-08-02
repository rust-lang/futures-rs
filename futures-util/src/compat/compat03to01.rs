use super::Compat;
use futures::{
    task as task01, Async as Async01, Future as Future01, Poll as Poll01, Stream as Stream01,
};
use futures_core::{task as task03, TryFuture as TryFuture03, TryStream as TryStream03};
use std::{marker::Unpin, mem::PinMut, sync::Arc};

impl<Fut, Ex> Future01 for Compat<Fut, Ex>
where
    Fut: TryFuture03 + Unpin,
    Ex: task03::Executor,
{
    type Item = Fut::Ok;
    type Error = Fut::Error;

    fn poll(&mut self) -> Poll01<Self::Item, Self::Error> {
        let waker = current_as_waker();
        let mut cx = task03::Context::new(&waker, self.executor.as_mut().unwrap());
        match PinMut::new(&mut self.inner).try_poll(&mut cx) {
            task03::Poll::Ready(Ok(t)) => Ok(Async01::Ready(t)),
            task03::Poll::Pending => Ok(Async01::NotReady),
            task03::Poll::Ready(Err(e)) => Err(e),
        }
    }
}

impl<St, Ex> Stream01 for Compat<St, Ex>
where
    St: TryStream03 + Unpin,
    Ex: task03::Executor,
{
    type Item = St::Ok;
    type Error = St::Error;

    fn poll(&mut self) -> Poll01<Option<Self::Item>, Self::Error> {
        let waker = current_as_waker();
        let mut cx = task03::Context::new(&waker, self.executor.as_mut().unwrap());
        match PinMut::new(&mut self.inner).try_poll_next(&mut cx) {
            task03::Poll::Ready(None) => Ok(Async01::Ready(None)),
            task03::Poll::Ready(Some(Ok(t))) => Ok(Async01::Ready(Some(t))),
            task03::Poll::Pending => Ok(Async01::NotReady),
            task03::Poll::Ready(Some(Err(e))) => Err(e),
        }
    }
}

fn current_as_waker() -> task03::LocalWaker {
    let arc_waker = Arc::new(Current(task01::current()));
    task03::local_waker_from_nonlocal(arc_waker)
}

struct Current(task01::Task);

impl task03::Wake for Current {
    fn wake(arc_self: &Arc<Self>) {
        arc_self.0.notify();
    }
}
