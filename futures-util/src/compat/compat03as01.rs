use futures_01::{
    task as task01, Async as Async01, AsyncSink as AsyncSink01,
    Future as Future01, Poll as Poll01, Sink as Sink01,
    StartSend as StartSend01, Stream as Stream01,
};
use futures_core::{
    task as task03, TryFuture as TryFuture03, TryStream as TryStream03,
};
use futures_sink::Sink as Sink03;
use std::{marker::Unpin, pin::Pin, ptr::NonNull, sync::Arc};

/// Converts a futures 0.3 [`TryFuture`](futures_core::future::TryFuture),
/// [`TryStream`](futures_core::stream::TryStream) or
/// [`Sink`](futures_sink::Sink) into a futures 0.1
/// [`Future`](futures::future::Future),
/// [`Stream`](futures::stream::Stream) or
/// [`Sink`](futures::sink::Sink).
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct Compat<T> {
    pub(crate) inner: T,
}

impl<T> Compat<T> {
    /// Returns the inner item.
    pub fn into_inner(self) -> T {
        self.inner
    }

    /// Creates a new [`Compat`].
    pub(crate) fn new(inner: T) -> Compat<T> {
        Compat { inner }
    }
}

fn poll_03_to_01<T, E>(x: task03::Poll<Result<T, E>>)
    -> Result<Async01<T>, E>
{
    match x {
        task03::Poll::Ready(Ok(t)) => Ok(Async01::Ready(t)),
        task03::Poll::Pending => Ok(Async01::NotReady),
        task03::Poll::Ready(Err(e)) => Err(e),
    }
}

impl<Fut> Future01 for Compat<Fut>
where
    Fut: TryFuture03 + Unpin,
{
    type Item = Fut::Ok;
    type Error = Fut::Error;

    fn poll(&mut self) -> Poll01<Self::Item, Self::Error> {
        with_context(self, |inner, lw| poll_03_to_01(inner.try_poll(lw)))
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
        with_context(self, |inner, lw| poll_03_to_01(inner.poll_flush(lw)))
    }

    fn close(&mut self) -> Poll01<(), Self::SinkError> {
        with_context(self, |inner, lw| poll_03_to_01(inner.poll_close(lw)))
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

#[cfg(feature = "io-compat")]
mod io {
    use super::*;
    use futures_io::{AsyncRead as AsyncRead03, AsyncWrite as AsyncWrite03};
    use tokio_io::{AsyncRead as AsyncRead01, AsyncWrite as AsyncWrite01};

    fn poll_03_to_io<T>(x: task03::Poll<Result<T, std::io::Error>>)
        -> Result<T, std::io::Error>
    {
        match x {
            task03::Poll::Ready(Ok(t)) => Ok(t),
            task03::Poll::Pending => Err(std::io::ErrorKind::WouldBlock.into()),
            task03::Poll::Ready(Err(e)) => Err(e),
        }
    }

    impl<R: AsyncRead03> std::io::Read for Compat<R> {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            let lw = current_ref_as_waker();
            poll_03_to_io(self.inner.poll_read(&lw, buf))
        }
    }

    impl<R: AsyncRead03> AsyncRead01 for Compat<R> {
        unsafe fn prepare_uninitialized_buffer(&self, buf: &mut [u8]) -> bool {
            let initializer = self.inner.initializer();
            let does_init = initializer.should_initialize();
            if does_init {
                initializer.initialize(buf);
            }
            does_init
        }
    }

    impl<W: AsyncWrite03> std::io::Write for Compat<W> {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            let lw = current_ref_as_waker();
            poll_03_to_io(self.inner.poll_write(&lw, buf))
        }

        fn flush(&mut self) -> std::io::Result<()> {
            let lw = current_ref_as_waker();
            poll_03_to_io(self.inner.poll_flush(&lw))
        }
    }

    impl<W: AsyncWrite03> AsyncWrite01 for Compat<W> {
        fn shutdown(&mut self) -> std::io::Result<Async01<()>> {
            let lw = current_ref_as_waker();
            poll_03_to_01(self.inner.poll_close(&lw))
        }
    }
}
