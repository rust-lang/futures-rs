//! Definition of the `PinnedFut` adapter combinator

// TODO: import `Pinned` and use it to make `Borrowed` immovable.
use core::mem::PinMut;

use futures_core::{Future, Poll};
use futures_core::task;

pub trait PinnedFnLt<'a, Data: 'a, Output> {
    type Future: Future<Output = Output> + 'a;
    fn apply(self, data: PinMut<'a, Data>) -> Self::Future;
}

pub trait PinnedFn<Data, Output>: for<'a> PinnedFnLt<'a, Data, Output> {}
impl<Data, Output, T> PinnedFn<Data, Output> for T
    where T: for<'a> PinnedFnLt<'a, Data, Output> {}

impl<'a, Data, Output, Fut, T> PinnedFnLt<'a, Data, Output> for T
where
    Data: 'a,
    T: FnOnce(PinMut<'a, Data>) -> Fut,
    Fut: Future<Output = Output> + 'a,
{
    type Future = Fut;
    fn apply(self, data: PinMut<'a, Data>) -> Self::Future {
        (self)(data)
    }
}

/// A future which borrows a value for an asynchronous lifetime.
///
/// Created by the `borrowed` function.
#[must_use = "futures do nothing unless polled"]
#[allow(missing_debug_implementations)]
pub struct PinnedFut<'any, Data: 'any, Output, F: PinnedFn<Data, Output> + 'any> {
    fn_or_fut: FnOrFut<'any, Data, Output, F>,
    // TODO:
    // marker: Pinned,
    // Data, which may be borrowed by `fn_or_fut`, must be dropped last
    data: Data,
}

enum FnOrFut<'any, Data: 'any, Output, F: PinnedFn<Data, Output> + 'any> {
    F(F),
    Fut(<F as PinnedFnLt<'any, Data, Output>>::Future),
    None,
}

impl<'any, Data: 'any, Output, F: PinnedFn<Data, Output> + 'any> FnOrFut<'any, Data, Output, F> {
    fn is_fn(&self) -> bool {
        if let FnOrFut::F(_) = self {
            true
        } else {
            false
        }
    }
}

/// Creates a new future which pins some data and borrows it for an
/// asynchronous lifetime.
pub fn pinned<'any, Data, Output, F>(data: Data, f: F) -> PinnedFut<'any, Data, Output, F>
    where F: PinnedFn<Data, Output> + 'any,
          Data: 'any,
{
    PinnedFut {
        fn_or_fut: FnOrFut::F(f),
        data,
    }
}

unsafe fn transmute_lt<'input, 'output, T>(x: &'input mut T) -> &'output mut T {
    ::std::mem::transmute(x)
}

impl<'any, Data, Output, F> Future for PinnedFut<'any, Data, Output, F>
    where F: PinnedFn<Data, Output> + 'any,
          Data: 'any,
{
    type Output = <<F as PinnedFnLt<'any, Data, Output>>::Future as Future>::Output;

    fn poll(self: PinMut<Self>, cx: &mut task::Context) -> Poll<Self::Output> {
        unsafe {
            let this = PinMut::get_mut(self);
            if this.fn_or_fut.is_fn() {
                if let FnOrFut::F(f) = ::std::mem::replace(&mut this.fn_or_fut, FnOrFut::None) {
                    let fut = f.apply(PinMut::new_unchecked(transmute_lt(&mut this.data)));
                    this.fn_or_fut = FnOrFut::Fut(fut);
                } else {
                    unreachable!()
                }
            }

            let res = if let FnOrFut::Fut(fut) = &mut this.fn_or_fut { 
                PinMut::new_unchecked(fut).poll(cx)
            } else {
                panic!("polled PinnedFut after completion")
            };

            if let Poll::Ready(_) = &res {
                this.fn_or_fut = FnOrFut::None;
            }

            res
        }
    }
}

#[allow(unused)]
fn does_compile() -> impl Future<Output = u8> {
    pinned(5, |x: PinMut<_>| { // This type annotation is *required* to compile
        ::future::lazy(move |_cx| {
            // we can use (copy from) the asynchronously borrowed data here
            *x
        })
    })
}
