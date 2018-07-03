use core::task::{TaskObj, LocalTaskObj};
use core::mem;
use std::boxed::{Box, PinBox};

use core::fmt;
use core::future::Future;
use core::marker::PhantomData;
use core::mem::PinMut;
use core::task::{Context, Poll};

/// A custom trait object for polling futures, roughly akin to
/// `Box<dyn Future<Output = T>>`.
/// Contrary to `FutureObj`, `LocalFutureObj` does not have a `Send` bound.
pub struct LocalFutureObj<'a, T> {
    ptr: *mut (),
    poll_fn: unsafe fn(*mut (), &mut Context) -> Poll<T>,
    drop_fn: unsafe fn(*mut ()),
    _marker1: PhantomData<T>,
    _marker2: PhantomData<&'a ()>,
}

impl<'a, T> LocalFutureObj<'a, T> {
    /// Create a `LocalFutureObj` from a custom trait object representation.
    #[inline]
    pub fn new<F: UnsafeFutureObj<'a, T> + 'a>(f: F) -> LocalFutureObj<'a, T> {
        LocalFutureObj {
            ptr: f.into_raw(),
            poll_fn: F::poll,
            drop_fn: F::drop,
            _marker1: PhantomData,
            _marker2: PhantomData,
        }
    }

    /// Converts the `LocalFutureObj` into a `FutureObj`
    /// To make this operation safe one has to ensure that the `UnsafeFutureObj`
    /// instance from which this `LocalFutureObj` was created actually
    /// implements `Send`.
    #[inline]
    pub unsafe fn into_future_obj(self) -> FutureObj<'a, T> {
        FutureObj(self)
    }
}

impl<'a, T> fmt::Debug for LocalFutureObj<'a, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("LocalFutureObj")
            .finish()
    }
}

impl<'a, T> From<FutureObj<'a, T>> for LocalFutureObj<'a, T> {
    #[inline]
    fn from(f: FutureObj<'a, T>) -> LocalFutureObj<'a, T> {
        f.0
    }
}

impl<'a, T> Future for LocalFutureObj<'a, T> {
    type Output = T;

    #[inline]
    fn poll(self: PinMut<Self>, cx: &mut Context) -> Poll<T> {
        unsafe {
            (self.poll_fn)(self.ptr, cx)
        }
    }
}

impl<'a, T> Drop for LocalFutureObj<'a, T> {
    fn drop(&mut self) {
        unsafe {
            (self.drop_fn)(self.ptr)
        }
    }
}

/// A custom trait object for polling futures, roughly akin to
/// `Box<dyn Future<Output = T>> + Send`.
pub struct FutureObj<'a, T>(LocalFutureObj<'a, T>);

unsafe impl<'a, T> Send for FutureObj<'a, T> {}

impl<'a, T> FutureObj<'a, T> {
    /// Create a `FutureObj` from a custom trait object representation.
    #[inline]
    pub fn new<F: UnsafeFutureObj<'a, T> + Send>(f: F) -> FutureObj<'a, T> {
        FutureObj(LocalFutureObj::new(f))
    }
}

impl<'a, T> fmt::Debug for FutureObj<'a, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("FutureObj")
            .finish()
    }
}

impl<'a, T> Future for FutureObj<'a, T> {
    type Output = T;

    #[inline]
    fn poll(self: PinMut<Self>, cx: &mut Context) -> Poll<T> {
        let pinned_field = unsafe { PinMut::map_unchecked(self, |x| &mut x.0) };
        pinned_field.poll(cx)
    }
}

/// A custom implementation of a future trait object for `FutureObj`, providing
/// a hand-rolled vtable.
///
/// This custom representation is typically used only in `no_std` contexts,
/// where the default `Box`-based implementation is not available.
///
/// The implementor must guarantee that it is safe to call `poll` repeatedly (in
/// a non-concurrent fashion) with the result of `into_raw` until `drop` is
/// called.
pub unsafe trait UnsafeFutureObj<'a, T>: 'a {
    /// Convert an owned instance into a (conceptually owned) void pointer.
    fn into_raw(self) -> *mut ();

    /// Poll the future represented by the given void pointer.
    ///
    /// # Safety
    ///
    /// The trait implementor must guarantee that it is safe to repeatedly call
    /// `poll` with the result of `into_raw` until `drop` is called; such calls
    /// are not, however, allowed to race with each other or with calls to `drop`.
    unsafe fn poll(future: *mut (), cx: &mut Context) -> Poll<T>;

    /// Drops the future represented by the given void pointer.
    ///
    /// # Safety
    ///
    /// The trait implementor must guarantee that it is safe to call this
    /// function once per `into_raw` invocation; that call cannot race with
    /// other calls to `drop` or `poll`.
    unsafe fn drop(future: *mut ());
}







impl From<FutureObj<'static, ()>> for TaskObj {
    #[inline]
    fn from(f: FutureObj<'static, ()>) -> TaskObj {
        unsafe { mem::transmute(f) }
    }
}

impl From<LocalFutureObj<'static, ()>> for LocalTaskObj {
    #[inline]
    fn from(f: LocalFutureObj<'static, ()>) -> LocalTaskObj {
        unsafe { mem::transmute(f) }
    }
}

impl From<TaskObj> for FutureObj<'static, ()> {
    #[inline]
    fn from(f: TaskObj) -> FutureObj<'static, ()> {
        unsafe { mem::transmute(f) }
    }
}

impl From<LocalTaskObj> for LocalFutureObj<'static, ()> {
    #[inline]
    fn from(f: LocalTaskObj) -> LocalFutureObj<'static, ()> {
        unsafe { mem::transmute(f) }
    }
}






unsafe impl<'a, T, F: Future<Output = T> + 'a> UnsafeFutureObj<'a, T> for PinBox<F> {
    fn into_raw(self) -> *mut () {
        PinBox::into_raw(self) as *mut ()
    }

    unsafe fn poll(task: *mut (), cx: &mut Context) -> Poll<T> {
        let ptr = task as *mut F;
        let pin: PinMut<F> = PinMut::new_unchecked(&mut *ptr);
        pin.poll(cx)
    }

    unsafe fn drop(task: *mut ()) {
        drop(PinBox::from_raw(task as *mut F))
    }
}

unsafe impl<'a, T, F: Future<Output = T> + 'a> UnsafeFutureObj<'a, T> for Box<F> {
    fn into_raw(self) -> *mut () {
        Box::into_raw(self) as *mut ()
    }

    unsafe fn poll(task: *mut (), cx: &mut Context) -> Poll<T> {
        let ptr = task as *mut F;
        let pin: PinMut<F> = PinMut::new_unchecked(&mut *ptr);
        pin.poll(cx)
    }

    unsafe fn drop(task: *mut ()) {
        drop(Box::from_raw(task as *mut F))
    }
}

impl<'a, F: Future<Output = ()> + Send + 'a> From<PinBox<F>> for FutureObj<'a, ()> {
    fn from(boxed: PinBox<F>) -> Self {
        FutureObj::new(boxed)
    }
}

impl<'a, F: Future<Output = ()> + Send + 'a> From<Box<F>> for FutureObj<'a, ()> {
    fn from(boxed: Box<F>) -> Self {
        FutureObj::new(PinBox::from(boxed))
    }
}

impl<'a, F: Future<Output = ()> + 'a> From<PinBox<F>> for LocalFutureObj<'a, ()> {
    fn from(boxed: PinBox<F>) -> Self {
        LocalFutureObj::new(boxed)
    }
}

impl<'a, F: Future<Output = ()> + 'a> From<Box<F>> for LocalFutureObj<'a, ()> {
    fn from(boxed: Box<F>) -> Self {
        LocalFutureObj::new(PinBox::from(boxed))
    }
}


unsafe impl<'a, T, F: Future<Output = T> + 'a> UnsafeFutureObj<'a, T> for PinMut<'a, F> {
    fn into_raw(self) -> *mut () {
        unsafe { PinMut::get_mut_unchecked(self) as *mut F as *mut () }
    }

    unsafe fn poll(ptr: *mut (), cx: &mut Context) -> Poll<T> {
        PinMut::new_unchecked(&mut *(ptr as *mut F)).poll(cx)
    }

    unsafe fn drop(_ptr: *mut ()) {}
}
