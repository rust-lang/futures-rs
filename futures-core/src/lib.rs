//! Core traits and types for asynchronous operations in Rust.

#![cfg_attr(feature = "nightly", feature(pin, arbitrary_self_types, specialization))]

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

extern crate parse_generics_shim;

macro_rules! if_std {
    ($($i:item)*) => ($(
        #[cfg(feature = "std")]
        $i
    )*)
}

#[macro_export]
macro_rules! pinned_deref {
    ($e:expr) => (
        ::core::mem::Pin::new_unchecked(
            &mut **::core::mem::Pin::get_mut(&mut $e)
        )
    )
}

#[macro_export]
macro_rules! pinned_field {
    ($e:expr, $f:tt) => (
        ::core::mem::Pin::new_unchecked(
            &mut ::core::mem::Pin::get_mut(&mut $e).$f
        )
    )
}

mod poll;
pub use poll::Poll;

pub mod future;
pub use future::Future;

pub mod stream;
pub use stream::Stream;

pub mod task;

pub mod executor;

/// Standin for the currently-unstable `std::marker::Unpin` trait
#[cfg(not(feature = "nightly"))]
pub unsafe trait Unpin {}
#[cfg(not(feature = "nightly"))]
mod impls;

#[cfg(feature = "nightly")]
pub use core::marker::Unpin;
#[cfg(feature = "nightly")]
mod impls_nightly;

#[allow(unused_imports)]
use parse_generics_shim::*;

#[macro_export]
macro_rules! unpinned {
    (impl $($tail:tt)*) => (unpinned! {
        @parse_generics $($tail)*
    });

    (@parse_generics $($toks:tt)*) => (parse_generics_shim! {
        { constr },
        then unpinned!(@parse_for),
        $($toks)*
    });

    (@parse_for
        { constr: [ $($constr:tt)* ], },
        $trait:ident for $t:ty where $($tail:tt)*
    ) => (parse_where_shim! {
        { clause, preds },
        then unpinned_emit!(
            trait: $trait,
            ty: $t,
            constr: [ $($constr)* ],
        ),
        where $($tail)*
    });

    (@parse_for
        { constr: [ $($constr:tt)* ], },
     $trait:ident for $t:ty { $($body:tt)* }
    ) => (unpinned_emit! {
        trait: $trait,
        ty: $t,
        constr: [ $($constr)* ],
        {
            clause: [],
            preds: [],
        },
        { $($body)* }
    });

}

#[macro_export]
#[cfg(feature = "nightly")]
macro_rules! unpinned_emit {
    (
        trait: Future,
        ty: $t:ty,
        constr: [ $($constr:tt)* ],
        {
            clause: [ $($clause:tt)* ],
            preds: [ $($preds:tt)* ],
        },
        { $($body:tt)* }
    ) => (
        unsafe impl<$($constr)*> $crate::Unpin for $t {}
        impl<$($constr)*> Future for $t $($clause)* {
            $($body)*

            fn poll(mut self: ::core::mem::Pin<Self>, cx: &mut task::Context) -> Poll<Self::Output> {
                self.poll_mut(cx)
            }
        }
    );

    (
        trait: Stream,
        ty: $t:ty,
        constr: [ $($constr:tt)* ],
        {
            clause: [ $($clause:tt)* ],
            preds: [ $($preds:tt)* ],
        },
        { $($body:tt)* }
    ) => (
        unsafe impl<$($constr)*> $crate::Unpin for $t {}
        impl<$($constr)*> Stream for $t $($clause)* {
            $($body)*

            fn poll_next(mut self: ::core::mem::Pin<Self>, cx: &mut task::Context) -> Poll<Option<Self::Item>> {
                self.poll_next_mut(cx)
            }
        }
    );
}

#[macro_export]
#[cfg(not(feature = "nightly"))]
macro_rules! unpinned_emit {
    (
        trait: $trait:ident,
        ty: $t:ty,
        constr: [ $($constr:tt)* ],
        {
            clause: [ $($clause:tt)* ],
            preds: [ $($preds:tt)* ],
        },
        { $($body:tt)* }
    ) => (
        unsafe impl<$($constr)*> $crate::Unpin for $t {}
        impl<$($constr)*> $trait for $t $($clause)* {
            $($body)*
        }
    )
}
