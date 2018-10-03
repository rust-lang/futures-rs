use super::Compat;
use futures::executor::{
    self as executor01, UnsafeNotify as UnsafeNotify01,
    Notify as Notify01, NotifyHandle as NotifyHandle01,
};
use futures::{Async as Async01, Future as Future01, Stream as Stream01};
use futures_core::{task as task03, Future as Future03, Stream as Stream03};
use std::mem;
use std::task::{LocalWaker, Waker};
use std::pin::Pin;

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

fn lw_as_notify<R>(lw: &LocalWaker, f: impl FnOnce() -> R) -> R {
    let notify = &WakerToHandle(local_as_waker(lw));
    executor01::with_notify(notify, 0, f)
}

impl<Fut: Future01> Future03 for Compat<Fut> {
    type Output = Result<Fut::Item, Fut::Error>;

    fn poll(
        self: Pin<&mut Self>,
        lw: &LocalWaker,
    ) -> task03::Poll<Self::Output> {
        lw_as_notify(lw, move || {
            poll_01_to_03(unsafe { Pin::get_mut_unchecked(self) }.inner.poll())
        })
    }
}

impl<St: Stream01> Stream03 for Compat<St> {
    type Item = Result<St::Item, St::Error>;

    fn poll_next(
        self: Pin<&mut Self>,
        lw: &LocalWaker,
    ) -> task03::Poll<Option<Self::Item>> {
        lw_as_notify(lw, move || {
            match unsafe { Pin::get_mut_unchecked(self) }.inner.poll() {
                Ok(Async01::Ready(Some(t))) => task03::Poll::Ready(Some(Ok(t))),
                Ok(Async01::Ready(None)) => task03::Poll::Ready(None),
                Ok(Async01::NotReady) => task03::Poll::Pending,
                Err(e) => task03::Poll::Ready(Some(Err(e))),
            }
        })
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

    impl<R: AsyncRead01> AsyncRead03 for Compat<R> {
        unsafe fn initializer(&self) -> Initializer {
            // check if `prepare_uninitialized_buffer` needs zeroing
            if self.inner.prepare_uninitialized_buffer(&mut [1]) {
                Initializer::zeroing()
            } else {
                Initializer::nop()
            }
        }

        fn poll_read(&mut self, lw: &task03::LocalWaker, buf: &mut [u8])
            -> task03::Poll<Result<usize, Error>>
        {
            lw_as_notify(lw, move || {
                poll_01_to_03(self.inner.poll_read(buf))
            })
        }
    }

    impl<W: AsyncWrite01> AsyncWrite03 for Compat<W> {
        fn poll_write(&mut self, lw: &task03::LocalWaker, buf: &[u8])
            -> task03::Poll<Result<usize, Error>>
        {
            lw_as_notify(lw, || poll_01_to_03(self.inner.poll_write(buf)))
        }

        fn poll_flush(&mut self, lw: &task03::LocalWaker)
            -> task03::Poll<Result<(), Error>>
        {
            lw_as_notify(lw, || poll_01_to_03(self.inner.poll_flush()))
        }

        fn poll_close(&mut self, lw: &task03::LocalWaker)
            -> task03::Poll<Result<(), Error>>
        {
            lw_as_notify(lw, || poll_01_to_03(self.inner.shutdown()))
        }
    }
}
