use super::Compat;
use futures::{
    task as task01, Async as Async01, AsyncSink as AsyncSink01,
    Future as Future01, Poll as Poll01, Sink as Sink01,
    StartSend as StartSend01, Stream as Stream01,
};
use futures_core::{
    task as task03, TryFuture as TryFuture03, TryStream as TryStream03,
};
use futures_sink::Sink as Sink03;
use std::{marker::Unpin, pin::Pin, ptr::NonNull, sync::Arc};

impl<Fut> Future01 for Compat<Fut>
where
    Fut: TryFuture03 + Unpin,
{
    type Item = Fut::Ok;
    type Error = Fut::Error;

    fn poll(&mut self) -> Poll01<Self::Item, Self::Error> {
        with_context(self, |inner, lw| match inner.try_poll(lw) {
            task03::Poll::Ready(Ok(t)) => Ok(Async01::Ready(t)),
            task03::Poll::Pending => Ok(Async01::NotReady),
            task03::Poll::Ready(Err(e)) => Err(e),
        })
    }
}

impl<St> Stream01 for Compat<St>
where
    St: TryStream03 + Unpin,
{
    type Item = St::Ok;
    type Error = St::Error;

    fn poll(&mut self) -> Poll01<Option<Self::Item>, Self::Error> {
        with_context(self, |inner, lw| match inner.try_poll_next(lw) {
            task03::Poll::Ready(None) => Ok(Async01::Ready(None)),
            task03::Poll::Ready(Some(Ok(t))) => Ok(Async01::Ready(Some(t))),
            task03::Poll::Pending => Ok(Async01::NotReady),
            task03::Poll::Ready(Some(Err(e))) => Err(e),
        })
    }
}

impl<T> Sink01 for Compat<T>
where
    T: Sink03 + Unpin,
{
    type SinkItem = T::SinkItem;
    type SinkError = T::SinkError;

    fn start_send(
        &mut self,
        item: Self::SinkItem,
    ) -> StartSend01<Self::SinkItem, Self::SinkError> {
        with_context(self, |mut inner, lw| {
            match inner.as_mut().poll_ready(lw) {
                task03::Poll::Ready(Ok(())) => {
                    inner.start_send(item).map(|()| AsyncSink01::Ready)
                }
                task03::Poll::Pending => Ok(AsyncSink01::NotReady(item)),
                task03::Poll::Ready(Err(e)) => Err(e),
            }
        })
    }

    fn poll_complete(&mut self) -> Poll01<(), Self::SinkError> {
        with_context(self, |inner, lw| match inner.poll_flush(lw) {
            task03::Poll::Ready(Ok(())) => Ok(Async01::Ready(())),
            task03::Poll::Pending => Ok(Async01::NotReady),
            task03::Poll::Ready(Err(e)) => Err(e),
        })
    }

    fn close(&mut self) -> Poll01<(), Self::SinkError> {
        with_context(self, |inner, lw| match inner.poll_close(lw) {
            task03::Poll::Ready(Ok(())) => Ok(Async01::Ready(())),
            task03::Poll::Pending => Ok(Async01::NotReady),
            task03::Poll::Ready(Err(e)) => Err(e),
        })
    }
}

fn current_ref_as_waker() -> task03::LocalWaker {
    unsafe {
        task03::LocalWaker::new(NonNull::<CurrentRef>::dangling())
    }
}

struct CurrentRef;

struct CurrentOwned(task01::Task);

unsafe impl task03::UnsafeWake for CurrentRef {
    #[inline]
    unsafe fn clone_raw(&self) -> task03::Waker {
        task03::Waker::from(Arc::new(CurrentOwned(task01::current())))
    }

    #[inline]
    unsafe fn drop_raw(&self) {} // Does nothing

    #[inline]
    unsafe fn wake(&self) {
        task01::current().notify();
    }
}

impl task03::Wake for CurrentOwned {
    fn wake(arc_self: &Arc<Self>) {
        arc_self.0.notify();
    }
}

fn with_context<T, R, F>(compat: &mut Compat<T>, f: F) -> R
where
    T: Unpin,
    F: FnOnce(Pin<&mut T>, &task03::LocalWaker) -> R,
{
    let lw = current_ref_as_waker();
    f(Pin::new(&mut compat.inner), &lw)
}
