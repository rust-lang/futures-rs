use futures_01::{
    task as task01, Async as Async01, AsyncSink as AsyncSink01,
    Future as Future01, Poll as Poll01, Sink as Sink01,
    StartSend as StartSend01, Stream as Stream01,
};
use futures_core::{
    task as task03, TryFuture as TryFuture03, TryStream as TryStream03,
};
use futures_sink::Sink as Sink03;
use crate::task::ArcWake as ArcWake03;
use std::{pin::Pin, sync::Arc};

/// Converts a futures 0.3 [`TryFuture`](futures_core::future::TryFuture),
/// [`TryStream`](futures_core::stream::TryStream) or
/// [`Sink`](futures_sink::Sink) into a futures 0.1
/// [`Future`](futures::future::Future),
/// [`Stream`](futures::stream::Stream) or
/// [`Sink`](futures::sink::Sink).
#[derive(Debug, Clone, Copy)]
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
    ///
    /// For types which implement appropriate futures `0.3`
    /// traits, the result will be a type which implements
    /// the corresponding futures 0.1 type.
    pub fn new(inner: T) -> Compat<T> {
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
        with_context(self, |inner, waker| poll_03_to_01(inner.try_poll(waker)))
    }
}

impl<St> Stream01 for Compat<St>
where
    St: TryStream03 + Unpin,
{
    type Item = St::Ok;
    type Error = St::Error;

    fn poll(&mut self) -> Poll01<Option<Self::Item>, Self::Error> {
        with_context(self, |inner, waker| match inner.try_poll_next(waker) {
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
        with_context(self, |mut inner, waker| {
            match inner.as_mut().poll_ready(waker) {
                task03::Poll::Ready(Ok(())) => {
                    inner.start_send(item).map(|()| AsyncSink01::Ready)
                }
                task03::Poll::Pending => Ok(AsyncSink01::NotReady(item)),
                task03::Poll::Ready(Err(e)) => Err(e),
            }
        })
    }

    fn poll_complete(&mut self) -> Poll01<(), Self::SinkError> {
        with_context(self, |inner, waker| poll_03_to_01(inner.poll_flush(waker)))
    }

    fn close(&mut self) -> Poll01<(), Self::SinkError> {
        with_context(self, |inner, waker| poll_03_to_01(inner.poll_close(waker)))
    }
}

struct Current(task01::Task);

impl Current {
    fn new() -> Current {
        Current(task01::current())
    }

    fn as_waker(&self) -> task03::Waker {
        // For simplicity reasons wrap the Waker into an Arc.
        // We can optimize this again later on and reintroduce WakerLt<'a> which
        // derefs to Waker, and where cloning it through RawWakerVTable returns
        // an Arc version
        ArcWake03::into_waker(Arc::new(Current(self.0.clone())))
    }
}

impl ArcWake03 for Current {
    fn wake(arc_self: &Arc<Self>) {
        arc_self.0.notify();
    }
}

fn with_context<T, R, F>(compat: &mut Compat<T>, f: F) -> R
where
    T: Unpin,
    F: FnOnce(Pin<&mut T>, &task03::Waker) -> R,
{
    let current = Current::new();
    let waker = current.as_waker();
    f(Pin::new(&mut compat.inner), &waker)
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
            let current = Current::new();
            let waker = current.as_waker();
            poll_03_to_io(self.inner.poll_read(&waker, buf))
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
            let current = Current::new();
            let waker = current.as_waker();
            poll_03_to_io(self.inner.poll_write(&waker, buf))
        }

        fn flush(&mut self) -> std::io::Result<()> {
            let current = Current::new();
            let waker = current.as_waker();
            poll_03_to_io(self.inner.poll_flush(&waker))
        }
    }

    impl<W: AsyncWrite03> AsyncWrite01 for Compat<W> {
        fn shutdown(&mut self) -> std::io::Result<Async01<()>> {
            let current = Current::new();
            let waker = current.as_waker();
            poll_03_to_01(self.inner.poll_close(&waker))
        }
    }
}
