use core::mem;
use core::pin::Pin;
use core::ptr;
use futures_core::future::{FusedFuture, Future};
use futures_core::task::{Context, Poll};
use pin_utils::{unsafe_pinned, unsafe_unpinned};

/// Future for the [`map`](super::FutureExt::map) method.
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct Map<Fut, F> {
    inner : Option<Inner<Fut, F>>,
}

#[derive(Debug)]
struct Inner<Fut, F> {
    future: Fut,
    f: F,
}

struct UnsafeSetNoneOnDrop<T>(*mut Option<T>);

impl<T> Drop for UnsafeSetNoneOnDrop<T> {
    fn drop(&mut self) {
        unsafe {
            ptr::write(self.0, None);
        }
    }
}

impl<Fut, F> Inner<Fut, F> {
    unsafe_pinned!(future: Fut);
    unsafe_unpinned!(f: F);

    /// Extracts the map function while dropping the future without violating
    /// the Pin guarantee like a simple Option::take would
    fn take_f(me: Pin<&mut Option<Inner<Fut, F>>>) -> Option<F> {
        unsafe {
            let opt = me.get_unchecked_mut();
            let opt_ptr : *mut Option<Inner<Fut, F>> = opt;
            match opt {
                None => None,
                Some(Inner { future, f }) => {
                    // clean up invalid state if future::drop() panics
                    let auto_take = UnsafeSetNoneOnDrop(opt_ptr);
                    ptr::drop_in_place(future);
                    let rv = ptr::read(f);
                    mem::drop(auto_take);
                    Some(rv)
                }
            }
        }
    }
}

impl<Fut, F> Map<Fut, F> {
    unsafe_pinned!(inner: Option<Inner<Fut, F>>);

    /// Creates a new Map.
    pub(super) fn new(future: Fut, f: F) -> Map<Fut, F> {
        Map { inner : Some(Inner { future, f }) }
    }
}

impl<Fut: Unpin, F> Unpin for Map<Fut, F> {}

impl<Fut, F, T> FusedFuture for Map<Fut, F>
    where Fut: Future,
          F: FnOnce(Fut::Output) -> T,
{
    fn is_terminated(&self) -> bool { self.inner.is_none() }
}

impl<Fut, F, T> Future for Map<Fut, F>
    where Fut: Future,
          F: FnOnce(Fut::Output) -> T,
{
    type Output = T;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<T> {
        let mut inner = self.inner();
        let output = ready!(inner
            .as_mut()
            .as_pin_mut()
            .expect("Map must not be polled after it returned `Poll::Ready`")
            .future()
            .poll(cx));
        let f = Inner::take_f(inner).unwrap();
        Poll::Ready(f(output))
    }
}
