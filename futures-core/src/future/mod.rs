//! Futures.

use core::mem::PinMut;
use core::marker::Unpin;

use Poll;
use task;

mod option;
pub use self::option::FutureOption;

#[cfg(feature = "either")]
mod either;

pub use core::future::Future;

/// Will probably merge with futures_util::FutureExt
pub trait CoreFutureExt: Future {
    /// A convenience for calling `Future::poll` on `Unpin` future types.
    fn poll_unpin(&mut self, cx: &mut task::Context) -> Poll<Self::Output>
        where Self: Unpin
    {
        PinMut::new(self).poll(cx)
    }
}

impl<T: ?Sized> CoreFutureExt for T where T: Future {}
 
// should be in std
// impl<'a, F: ?Sized + Future + Unpin> Future for &'a mut F {
//     type Output = F::Output;

//     fn poll(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<Self::Output> {
//         F::poll(PinMut::new(&mut **self), cx)
//     }
// }

// impl<'a, F: ?Sized + Future> Future for PinMut<'a, F> {
//     type Output = F::Output;

//     fn poll(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<Self::Output> {
//         F::poll((*self).reborrow(), cx)
//     }
// }

// if_std! {
//     use std::boxed::{Box, PinBox};

//     impl<'a, F: ?Sized + Future + Unpin> Future for Box<F> {
//         type Output = F::Output;

//         fn poll(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<Self::Output> {
//             (**self).poll_unpin(cx)
//         }
//     }

//     impl<'a, F: ?Sized + Future> Future for PinBox<F> {
//         type Output = F::Output;

//         fn poll(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<Self::Output> {
//             self.as_pin_mut().poll(cx)
//         }
//     }

//     impl<'a, F: Future> Future for ::std::panic::AssertUnwindSafe<F> {
//         type Output = F::Output;

//         fn poll(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<Self::Output> {
//             unsafe { pinned_field!(self, 0).poll(cx) }
//         }
//     }
// }

/// A convenience for futures that return `Result` values that includes
/// a variety of adapters tailored to such futures.
pub trait TryFuture {
    /// The type of successful values yielded by this future
    type Item;

    /// The type of failures yielded by this future
    type Error;

    /// Poll this `TryFuture` as if it were a `Future`.
    ///
    /// This method is a stopgap for a compiler limitation that prevents us from
    /// directly inheriting from the `Future` trait; in the future it won't be
    /// needed.
    fn try_poll(self: PinMut<Self>, cx: &mut task::Context) -> Poll<Result<Self::Item, Self::Error>>;
}

impl<F, T, E> TryFuture for F
    where F: Future<Output = Result<T, E>>
{
    type Item = T;
    type Error = E;

    fn try_poll(self: PinMut<Self>, cx: &mut task::Context) -> Poll<F::Output> {
        self.poll(cx)
    }
}

/// A future that is immediately ready with a value
#[derive(Debug, Clone)]
#[must_use = "futures do nothing unless polled"]
pub struct ReadyFuture<T>(Option<T>);

impl<T> Unpin for ReadyFuture<T> {}

impl<T> Future for ReadyFuture<T> {
    type Output = T;

    fn poll(mut self: PinMut<Self>, _cx: &mut task::Context) -> Poll<T> {
        Poll::Ready(self.0.take().unwrap())
    }
}

/// Create a future that is immediately ready with a value.
pub fn ready<T>(t: T) -> ReadyFuture<T> {
    ReadyFuture(Some(t))
}
