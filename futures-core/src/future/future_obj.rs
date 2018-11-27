use core::{
    fmt,
    future::Future,
    marker::PhantomData,
    pin::Pin,
    task::{Context, Poll},
};

/// A custom trait object for polling futures, roughly akin to
/// `Box<dyn Future<Output = T> + 'a>`.
///
/// This custom trait object was introduced as currently it is not possible to
/// take `dyn Trait` by value and `Box<dyn Trait>` is not available in no_std
/// contexts.
pub struct LocalFutureObj<'a, T> {
    ptr: *mut (),
    poll_fn: unsafe fn(*mut (), &mut Context<'_>) -> Poll<T>,
    drop_fn: unsafe fn(*mut ()),
    _marker: PhantomData<&'a ()>,
}

impl<T> Unpin for LocalFutureObj<'_, T> {}

impl<'a, T> LocalFutureObj<'a, T> {
    /// Create a `LocalFutureObj` from a custom trait object representation.
    #[inline]
    pub fn new<F: UnsafeFutureObj<'a, T> + 'a>(f: F) -> LocalFutureObj<'a, T> {
        LocalFutureObj {
            ptr: f.into_raw(),
            poll_fn: F::poll,
            drop_fn: F::drop,
            _marker: PhantomData,
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

impl<T> fmt::Debug for LocalFutureObj<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
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

impl<T> Future for LocalFutureObj<'_, T> {
    type Output = T;

    #[inline]
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<T> {
        unsafe {
            ((*self).poll_fn)((*self).ptr, cx)
        }
    }
}

impl<T> Drop for LocalFutureObj<'_, T> {
    fn drop(&mut self) {
        unsafe {
            (self.drop_fn)(self.ptr)
        }
    }
}

/// A custom trait object for polling futures, roughly akin to
/// `Box<dyn Future<Output = T> + Send + 'a>`.
///
/// This custom trait object was introduced as currently it is not possible to
/// take `dyn Trait` by value and `Box<dyn Trait>` is not available in no_std
/// contexts.
///
/// You should generally not need to use this type outside of `no_std` or when
/// implementing `Spawn`, consider using [`BoxFuture`](crate::future::BoxFuture)
/// instead.
pub struct FutureObj<'a, T>(LocalFutureObj<'a, T>);

impl<T> Unpin for FutureObj<'_, T> {}
unsafe impl<T> Send for FutureObj<'_, T> {}

impl<'a, T> FutureObj<'a, T> {
    /// Create a `FutureObj` from a custom trait object representation.
    #[inline]
    pub fn new<F: UnsafeFutureObj<'a, T> + Send>(f: F) -> FutureObj<'a, T> {
        FutureObj(LocalFutureObj::new(f))
    }
}

impl<T> fmt::Debug for FutureObj<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FutureObj")
            .finish()
    }
}

impl<T> Future for FutureObj<'_, T> {
    type Output = T;

    #[inline]
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<T> {
        let pinned_field: Pin<&mut LocalFutureObj<'_, T>> = unsafe {
            Pin::map_unchecked_mut(self, |x| &mut x.0)
        };
        LocalFutureObj::poll(pinned_field, cx)
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
    /// are not, however, allowed to race with each other or with calls to
    /// `drop`.
    unsafe fn poll(ptr: *mut (), cx: &mut Context<'_>) -> Poll<T>;

    /// Drops the future represented by the given void pointer.
    ///
    /// # Safety
    ///
    /// The trait implementor must guarantee that it is safe to call this
    /// function once per `into_raw` invocation; that call cannot race with
    /// other calls to `drop` or `poll`.
    unsafe fn drop(ptr: *mut ());
}

unsafe impl<'a, T, F> UnsafeFutureObj<'a, T> for &'a mut F
where
    F: Future<Output = T> + Unpin + 'a
{
    fn into_raw(self) -> *mut () {
        self as *mut F as *mut ()
    }

    unsafe fn poll(ptr: *mut (), cx: &mut Context<'_>) -> Poll<T> {
        let p: Pin<&mut F> = Pin::new_unchecked(&mut *(ptr as *mut F));
        F::poll(p, cx)
    }

    unsafe fn drop(_ptr: *mut ()) {}
}

unsafe impl<'a, T, F> UnsafeFutureObj<'a, T> for Pin<&'a mut F>
where
    F: Future<Output = T> + 'a
{
    fn into_raw(mut self) -> *mut () {
        let mut_ref: &mut F = unsafe { Pin::get_unchecked_mut(self.as_mut()) };
        mut_ref as *mut F as *mut ()
    }

    unsafe fn poll(ptr: *mut (), cx: &mut Context<'_>) -> Poll<T> {
        let future: Pin<&mut F> = Pin::new_unchecked(&mut *(ptr as *mut F));
        F::poll(future, cx)
    }

    unsafe fn drop(_ptr: *mut ()) {}
}

#[cfg(feature = "alloc")]
mod if_alloc {
    use super::*;
    use core::mem;
    use alloc::boxed::Box;

    unsafe impl<'a, T, F> UnsafeFutureObj<'a, T> for Box<F>
        where F: Future<Output = T> + 'a
    {
        fn into_raw(self) -> *mut () {
            Box::into_raw(self) as *mut ()
        }

        unsafe fn poll(ptr: *mut (), cx: &mut Context<'_>) -> Poll<T> {
            let ptr = ptr as *mut F;
            let pin: Pin<&mut F> = Pin::new_unchecked(&mut *ptr);
            F::poll(pin, cx)
        }

        unsafe fn drop(ptr: *mut ()) {
            drop(Box::from_raw(ptr as *mut F))
        }
    }

    unsafe impl<'a, T, F> UnsafeFutureObj<'a, T> for Pin<Box<F>>
    where
        F: Future<Output = T> + 'a
    {
        fn into_raw(mut self) -> *mut () {
            let mut_ref: &mut F = unsafe { Pin::get_unchecked_mut(self.as_mut()) };
            let ptr = mut_ref as *mut F as *mut ();
            mem::forget(self); // Don't drop the box
            ptr
        }

        unsafe fn poll(ptr: *mut (), cx: &mut Context<'_>) -> Poll<T> {
            let ptr = ptr as *mut F;
            let pin: Pin<&mut F> = Pin::new_unchecked(&mut *ptr);
            F::poll(pin, cx)
        }

        unsafe fn drop(ptr: *mut ()) {
            #[allow(clippy::cast_ptr_alignment)]
            drop(Pin::from(Box::from_raw(ptr as *mut F)));
        }
    }

    impl<'a, F: Future<Output = ()> + Send + 'a> From<Pin<Box<F>>> for FutureObj<'a, ()> {
        fn from(boxed: Pin<Box<F>>) -> Self {
            FutureObj::new(boxed)
        }
    }

    impl<'a, F: Future<Output = ()> + Send + 'a> From<Box<F>> for FutureObj<'a, ()> {
        fn from(boxed: Box<F>) -> Self {
            FutureObj::new(boxed)
        }
    }

    impl<'a, F: Future<Output = ()> + 'a> From<Pin<Box<F>>> for LocalFutureObj<'a, ()> {
        fn from(boxed: Pin<Box<F>>) -> Self {
            LocalFutureObj::new(boxed)
        }
    }

    impl<'a, F: Future<Output = ()> + 'a> From<Box<F>> for LocalFutureObj<'a, ()> {
        fn from(boxed: Box<F>) -> Self {
            LocalFutureObj::new(boxed)
        }
    }
}
