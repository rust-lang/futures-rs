//! Asynchronous values.
//!
//! This module contains:
//!
//! - The [`Future`] trait.
//! - The [`FutureExt`] and [`TryFutureExt`] trait, which provides adapters for
//!   chaining and composing futures.
//! - Top-level future combinators like [`lazy`](lazy()) which creates a future
//!   from a closure that defines its return value, and [`ready`](ready()),
//!   which constructs a future with an immediate defined value.

use core::pin::Pin;
use futures_core::task::{Poll, Context};
#[cfg(feature = "alloc")]
use alloc::boxed::Box;

#[doc(no_inline)]
pub use core::future::Future;

pub use futures_core::future::FusedFuture;
pub use futures_task::{FutureObj, LocalFutureObj, UnsafeFutureObj};

/// An owned dynamically typed [`Future`] for use in cases where you can't
/// statically type your result or need to add some indirection.
#[cfg(feature = "alloc")]
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// `BoxFuture`, but without the `Send` requirement.
#[cfg(feature = "alloc")]
pub type LocalBoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + 'a>>;


mod private_try_future {
    use super::Future;

    pub trait Sealed {}

    impl<F, T, E> Sealed for F where F: ?Sized + Future<Output = Result<T, E>> {}
}

/// A convenience for futures that return `Result` values that includes
/// a variety of adapters tailored to such futures.
pub trait TryFuture: Future + private_try_future::Sealed {
    /// The type of successful values yielded by this future
    type Ok;

    /// The type of failures yielded by this future
    type Error;

    /// Poll this `TryFuture` as if it were a `Future`.
    ///
    /// This method is a stopgap for a compiler limitation that prevents us from
    /// directly inheriting from the `Future` trait; in the future it won't be
    /// needed.
    fn try_poll(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<Self::Ok, Self::Error>>;
}

impl<F, T, E> TryFuture for F
    where F: ?Sized + Future<Output = Result<T, E>>
{
    type Ok = T;
    type Error = E;

    #[inline]
    fn try_poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.poll(cx)
    }
}

// Extension traits and combinators
#[allow(clippy::module_inception)]
mod future;
pub use self::future::{
    Flatten, Fuse, FutureExt, Inspect, IntoStream, Map, NeverError, Then, UnitError, MapInto,
};

#[deprecated(note = "This is now an alias for [Flatten](Flatten)")]
pub use self::future::FlattenStream;

#[cfg(feature = "std")]
pub use self::future::CatchUnwind;

#[cfg(feature = "channel")]
#[cfg_attr(docsrs, doc(cfg(feature = "channel")))]
#[cfg(feature = "std")]
pub use self::future::{Remote, RemoteHandle};

#[cfg(feature = "std")]
pub use self::future::{Shared, WeakShared};

mod try_future;
pub use self::try_future::{
    AndThen, ErrInto, OkInto, InspectErr, InspectOk, IntoFuture, MapErr, MapOk, OrElse, TryFlattenStream,
    TryFutureExt, UnwrapOrElse, MapOkOrElse, TryFlatten,
};

#[cfg(feature = "sink")]
#[cfg_attr(docsrs, doc(cfg(feature = "sink")))]
pub use self::try_future::FlattenSink;

// Primitive futures

mod lazy;
pub use self::lazy::{lazy, Lazy};

mod pending;
pub use self::pending::{pending, Pending};

mod maybe_done;
pub use self::maybe_done::{maybe_done, MaybeDone};

mod try_maybe_done;
pub use self::try_maybe_done::{try_maybe_done, TryMaybeDone};

mod option;
pub use self::option::OptionFuture;

mod poll_fn;
pub use self::poll_fn::{poll_fn, PollFn};

mod ready;
pub use self::ready::{err, ok, ready, Ready};

mod join;
pub use self::join::{join, join3, join4, join5, Join, Join3, Join4, Join5};

#[cfg(feature = "alloc")]
mod join_all;
#[cfg(feature = "alloc")]
pub use self::join_all::{join_all, JoinAll};

mod select;
pub use self::select::{select, Select};

#[cfg(feature = "alloc")]
mod select_all;
#[cfg(feature = "alloc")]
pub use self::select_all::{select_all, SelectAll};

mod try_join;
pub use self::try_join::{
    try_join, try_join3, try_join4, try_join5, TryJoin, TryJoin3, TryJoin4, TryJoin5,
};

#[cfg(feature = "alloc")]
mod try_join_all;
#[cfg(feature = "alloc")]
pub use self::try_join_all::{try_join_all, TryJoinAll};

mod try_select;
pub use self::try_select::{try_select, TrySelect};

#[cfg(feature = "alloc")]
mod select_ok;
#[cfg(feature = "alloc")]
pub use self::select_ok::{select_ok, SelectOk};

mod either;
pub use self::either::Either;

cfg_target_has_atomic! {
    #[cfg(feature = "alloc")]
    mod abortable;
    #[cfg(feature = "alloc")]
    pub use self::abortable::{abortable, Abortable, AbortHandle, AbortRegistration, Aborted};
}

// Just a helper function to ensure the futures we're returning all have the
// right implementations.
pub(crate) fn assert_future<T, F>(future: F) -> F
where
    F: Future<Output = T>,
{
    future
}
