//! Futures 0.1 / 0.3 shims
//! 

#![allow(missing_debug_implementations)]

use futures::Future as Future01;
use futures::Poll as Poll01;
use futures::task as task01;
use futures::task::Task as Task01;
use futures::executor::{with_notify, NotifyHandle, Notify, UnsafeNotify};

use futures_core::Future as Future03;
use futures_core::TryFuture as TryFuture03;
use futures_core::Poll as Poll03;
use futures_core::task;
use futures_core::task::Executor as Executor03;
use futures_core::task::{Wake, Waker, LocalWaker, local_waker_from_nonlocal};

use core::mem::PinMut;
use core::marker::Unpin;
use std::sync::Arc;

/// Converts a futures 0.3 `TryFuture` into a futures 0.1 `Future`
/// and vice versa.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct Compat<Fut, E> {
    crate inner: Fut, 
    crate executor: Option<E>,
}

impl<Fut, E> Compat<Fut, E> {
    /// Returns the inner future.
    pub fn into_inner(self) -> Fut {
        self.inner
    }

    /// Creates a new `Compat`.
    crate fn new(inner: Fut, executor: Option<E>) -> Compat<Fut, E> {
        Compat {
            inner,
            executor
        }
    }
}

impl<T> Future03 for Compat<T, ()> where T: Future01 {
    type Output = Result<T::Item, T::Error>;

    fn poll(self: PinMut<Self>, cx: &mut task::Context) -> Poll03<Self::Output> {
        use futures::Async;

        let notify = &WakerToHandle(cx.waker());

        with_notify(notify, 0, move || { 
            unsafe {
                match PinMut::get_mut_unchecked(self).inner.poll() {
                    Ok(Async::Ready(t)) => Poll03::Ready(Ok(t)),
                    Ok(Async::NotReady) => Poll03::Pending,
                    Err(e) => Poll03::Ready(Err(e)),
                }
            }
        })
    }
}

struct NotifyWaker(Waker);

#[derive(Clone)]
struct WakerToHandle<'a>(&'a Waker);

impl<'a> From<WakerToHandle<'a>> for NotifyHandle {
    fn from(handle: WakerToHandle<'a>) -> NotifyHandle {
        let ptr = Box::new(NotifyWaker(handle.0.clone()));

        unsafe {
            NotifyHandle::new(Box::into_raw(ptr))
        }
    }
}

impl Notify for NotifyWaker {
    fn notify(&self, _: usize) {
        self.0.wake();
    }
}

unsafe impl UnsafeNotify for NotifyWaker {
    unsafe fn clone_raw(&self) -> NotifyHandle {
        WakerToHandle(&self.0).into()
    }

    unsafe fn drop_raw(&self) {
        let ptr: *const dyn UnsafeNotify = self;
        drop(Box::from_raw(ptr as *mut dyn UnsafeNotify));
    }
}




impl<T, E> Future01 for Compat<T, E> where T: TryFuture03 + Unpin,
    E: Executor03
{
    type Item = T::Ok;
    type Error = T::Error;

    fn poll(&mut self) -> Poll01<Self::Item, Self::Error> {
        use futures::Async;

        let waker = current_as_waker();
        let mut cx = task::Context::new(&waker, self.executor.as_mut().unwrap());
        match PinMut::new(&mut self.inner).try_poll(&mut cx) {
            Poll03::Ready(Ok(t)) => Ok(Async::Ready(t)),
            Poll03::Pending => Ok(Async::NotReady),
            Poll03::Ready(Err(e)) => Err(e),
        }
    }
}

fn current_as_waker() -> LocalWaker {
    let arc_waker = Arc::new(Current(task01::current()));
    local_waker_from_nonlocal(arc_waker)
}

struct Current(Task01);

impl Wake for Current {
    fn wake(arc_self: &Arc<Self>) {
        arc_self.0.notify();
    }
}
