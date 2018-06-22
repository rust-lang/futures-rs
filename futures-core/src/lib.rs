//! Core traits and types for asynchronous operations in Rust.

#![feature(pin, arbitrary_self_types)]
#![feature(futures_api)]

#![no_std]
#![deny(missing_docs, missing_debug_implementations, warnings)]
#![doc(html_root_url = "https://docs.rs/futures-core/0.3.0")]

#![cfg_attr(feature = "nightly", feature(cfg_target_has_atomic))]
#![cfg_attr(feature = "nightly", feature(pin))]

#[macro_use]
#[cfg(feature = "std")]
extern crate std;
#[cfg(feature = "either")]
extern crate either;

macro_rules! if_std {
    ($($i:item)*) => ($(
        #[cfg(feature = "std")]
        $i
    )*)
}

#[macro_export]
macro_rules! pinned_deref {
    ($e:expr) => (
        ::core::mem::PinMut::new_unchecked(
            &mut **::core::mem::PinMut::get_mut($e.reborrow())
        )
    )
}

#[macro_export]
macro_rules! pinned_field {
    ($e:expr, $f:tt) => (
        ::core::mem::PinMut::new_unchecked(
            &mut ::core::mem::PinMut::get_mut($e.reborrow()).$f
        )
    )
}

#[macro_export]
macro_rules! unsafe_pinned {
    ($f:tt -> $t:ty) => (
        fn $f<'a>(self: &'a mut PinMut<Self>) -> PinMut<'a, $t> {
            unsafe {
                pinned_field!(self, $f)
            }
        }
    )
}

#[macro_export]
macro_rules! unsafe_unpinned {
    ($f:tt -> $t:ty) => (
        fn $f<'a>(self: &'a mut PinMut<Self>) -> &'a mut $t {
            unsafe {
                &mut ::core::mem::PinMut::get_mut(self.reborrow()).$f
            }
        }
    )
}

#[macro_export]
macro_rules! pin_mut {
    ($($x:ident),*) => { $(
        // Move the value to ensure that it is owned
        let mut $x = $x;
        // Shadow the original binding so that it can't be directly accessed
        // ever again.
        #[allow(unused_mut)]
        let mut $x = unsafe { ::std::mem::PinMut::new_unchecked(&mut $x) };
    )* }
}

pub mod future;
pub use future::{Future, CoreFutureExt, TryFuture};

pub mod stream;
pub use stream::Stream;

pub mod task;
pub use task::Poll;

pub mod executor;
