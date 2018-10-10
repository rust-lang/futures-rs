use futures_01::executor::{
    Spawn as Spawn01, spawn as spawn01,
    UnsafeNotify as UnsafeNotify01,
    Notify as Notify01,
    NotifyHandle as NotifyHandle01,
};
use futures_01::{Async as Async01, Future as Future01, Stream as Stream01};
use futures_core::{task as task03, Future as Future03, Stream as Stream03};
use std::mem;
use std::task::{LocalWaker, Waker};
use std::pin::{Pin, Unpin};

/// Converts a futures 0.1 Future, Stream, AsyncRead, or AsyncWrite
/// object to a futures 0.3-compatible version,
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct Compat01As03<T> {
    pub(crate) inner: Spawn01<T>,
}

impl<T> Unpin for Compat01As03<T> {}

impl<T> Compat01As03<T> {
    /// Wraps a futures 0.1 Future, Stream, AsyncRead, or AsyncWrite
    /// object in a futures 0.3-compatible wrapper.
    pub fn new(object: T) -> Compat01As03<T> {
        Compat01As03 {
            inner: spawn01(object),
        }
    }

    fn in_notify<R>(&mut self, lw: &LocalWaker, f: impl FnOnce(&mut T) -> R) -> R {
        let notify = &WakerToHandle(local_as_waker(lw));
        self.inner.poll_fn_notify(notify, 0, f)
    }
}

/// Extension trait for futures 0.1 [`Future`](futures::future::Future)
pub trait Future01CompatExt: Future01 {
    /// Converts a futures 0.1
    /// [`Future<Item = T, Error = E>`](futures::future::Future)
    /// into a futures 0.3
    /// [`Future<Output = Result<T, E>>`](futures_core::future::Future).
    fn compat(self) -> Compat01As03<Self> where Self: Sized {
        Compat01As03::new(self)
    }
}
impl<Fut: Future01> Future01CompatExt for Fut {}

/// Extension trait for futures 0.1 [`Stream`](futures::stream::Stream)
pub trait Stream01CompatExt: Stream01 {
    /// Converts a futures 0.1
    /// [`Stream<Item = T, Error = E>`](futures::stream::Stream)
    /// into a futures 0.3
    /// [`Stream<Item = Result<T, E>>`](futures_core::stream::Stream).
    fn compat(self) -> Compat01As03<Self> where Self: Sized {
        Compat01As03::new(self)
    }
}
impl<St: Stream01> Stream01CompatExt for St {}

// TODO(cramertj) use as_waker from std when it lands
fn local_as_waker(lw: &LocalWaker) -> &Waker {
    unsafe { mem::transmute(lw) }
}

fn poll_01_to_03<T, E>(x: Result<Async01<T>, E>)
    -> task03::Poll<Result<T, E>>
{
    match x {
        Ok(Async01::Ready(t)) => task03::Poll::Ready(Ok(t)),
        Ok(Async01::NotReady) => task03::Poll::Pending,
        Err(e) => task03::Poll::Ready(Err(e)),
    }
}

impl<Fut: Future01> Future03 for Compat01As03<Fut> {
    type Output = Result<Fut::Item, Fut::Error>;

    fn poll(
        mut self: Pin<&mut Self>,
        lw: &LocalWaker,
    ) -> task03::Poll<Self::Output> {
        poll_01_to_03(self.in_notify(lw, |f| f.poll()))
    }
}

impl<St: Stream01> Stream03 for Compat01As03<St> {
    type Item = Result<St::Item, St::Error>;

    fn poll_next(
        mut self: Pin<&mut Self>,
        lw: &LocalWaker,
    ) -> task03::Poll<Option<Self::Item>> {
        match self.in_notify(lw, |f| f.poll()) {
            Ok(Async01::Ready(Some(t))) => task03::Poll::Ready(Some(Ok(t))),
            Ok(Async01::Ready(None)) => task03::Poll::Ready(None),
            Ok(Async01::NotReady) => task03::Poll::Pending,
            Err(e) => task03::Poll::Ready(Some(Err(e))),
        }
    }
}

struct NotifyWaker(task03::Waker);

#[derive(Clone)]
struct WakerToHandle<'a>(&'a task03::Waker);

impl<'a> From<WakerToHandle<'a>> for NotifyHandle01 {
    fn from(handle: WakerToHandle<'a>) -> NotifyHandle01 {
        let ptr = Box::new(NotifyWaker(handle.0.clone()));

        unsafe { NotifyHandle01::new(Box::into_raw(ptr)) }
    }
}

impl Notify01 for NotifyWaker {
    fn notify(&self, _: usize) {
        self.0.wake();
    }
}

unsafe impl UnsafeNotify01 for NotifyWaker {
    unsafe fn clone_raw(&self) -> NotifyHandle01 {
        WakerToHandle(&self.0).into()
    }

    unsafe fn drop_raw(&self) {
        let ptr: *const dyn UnsafeNotify01 = self;
        drop(Box::from_raw(ptr as *mut dyn UnsafeNotify01));
    }
}

#[cfg(feature = "io-compat")]
mod io {
    use super::*;
    use futures_io::{
        AsyncRead as AsyncRead03,
        AsyncWrite as AsyncWrite03,
        Initializer,
    };
    use std::io::Error;
    use tokio_io::{AsyncRead as AsyncRead01, AsyncWrite as AsyncWrite01};

    impl<R: AsyncRead01> AsyncRead03 for Compat01As03<R> {
        unsafe fn initializer(&self) -> Initializer {
            // check if `prepare_uninitialized_buffer` needs zeroing
            if self.inner.get_ref().prepare_uninitialized_buffer(&mut [1]) {
                Initializer::zeroing()
            } else {
                Initializer::nop()
            }
        }

        fn poll_read(&mut self, lw: &task03::LocalWaker, buf: &mut [u8])
            -> task03::Poll<Result<usize, Error>>
        {
            poll_01_to_03(self.in_notify(lw, |x| x.poll_read(buf)))
        }
    }

    impl<W: AsyncWrite01> AsyncWrite03 for Compat01As03<W> {
        fn poll_write(&mut self, lw: &task03::LocalWaker, buf: &[u8])
            -> task03::Poll<Result<usize, Error>>
        {
            poll_01_to_03(self.in_notify(lw, |x| x.poll_write(buf)))
        }

        fn poll_flush(&mut self, lw: &task03::LocalWaker)
            -> task03::Poll<Result<(), Error>>
        {
            poll_01_to_03(self.in_notify(lw, |x| x.poll_flush()))
        }

        fn poll_close(&mut self, lw: &task03::LocalWaker)
            -> task03::Poll<Result<(), Error>>
        {
            poll_01_to_03(self.in_notify(lw, |x| x.shutdown()))
        }
    }
}
