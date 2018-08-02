use super::Compat;
use futures::{
    executor::{
        self as executor01, Notify as Notify01, NotifyHandle as NotifyHandle01,
        UnsafeNotify as UnsafeNotify01,
    },
    Async as Async01, Future as Future01, Stream as Stream01,
};
use futures_core::{task as task03, Future as Future03, Stream as Stream03};
use std::mem::PinMut;

impl<Fut: Future01> Future03 for Compat<Fut, ()> {
    type Output = Result<Fut::Item, Fut::Error>;

    fn poll(self: PinMut<Self>, cx: &mut task03::Context) -> task03::Poll<Self::Output> {
        let notify = &WakerToHandle(cx.waker());

        executor01::with_notify(notify, 0, move || {
            match unsafe { PinMut::get_mut_unchecked(self) }.inner.poll() {
                Ok(Async01::Ready(t)) => task03::Poll::Ready(Ok(t)),
                Ok(Async01::NotReady) => task03::Poll::Pending,
                Err(e) => task03::Poll::Ready(Err(e)),
            }
        })
    }
}

impl<St: Stream01> Stream03 for Compat<St, ()> {
    type Item = Result<St::Item, St::Error>;

    fn poll_next(self: PinMut<Self>, cx: &mut task03::Context) -> task03::Poll<Option<Self::Item>> {
        let notify = &WakerToHandle(cx.waker());

        executor01::with_notify(notify, 0, move || {
            match unsafe { PinMut::get_mut_unchecked(self) }.inner.poll() {
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
