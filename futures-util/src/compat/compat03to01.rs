use super::Compat;
use futures::Future as Future01;
use futures::Poll as Poll01;
use futures::task as task01;
use futures::Async as Async01;
use futures_core::TryFuture as TryFuture03;
use futures_core::task as task03;
use std::marker::Unpin;
use std::mem::PinMut;
use std::sync::Arc;

impl<Fut, Ex> Future01 for Compat<Fut, Ex>
where Fut: TryFuture03 + Unpin,
      Ex: task03::Executor
{
    type Item = Fut::Ok;
    type Error = Fut::Error;

    fn poll(&mut self) -> Poll01<Self::Item, Self::Error> {
        let waker = current_as_waker();
        let mut cx = task03::Context::new(&waker, self.executor.as_mut().unwrap());
        match PinMut::new(&mut self.future).try_poll(&mut cx) {
            task03::Poll::Ready(Ok(t)) => Ok(Async01::Ready(t)),
            task03::Poll::Pending => Ok(Async01::NotReady),
            task03::Poll::Ready(Err(e)) => Err(e),
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
