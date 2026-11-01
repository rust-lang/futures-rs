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

#[doc(no_inline)]
pub use core::future::Future;

#[cfg(feature = "alloc")]
pub use futures_core::future::{BoxFuture, LocalBoxFuture};
pub use futures_core::future::{FusedFuture, TryFuture};
pub use futures_task::{FutureObj, LocalFutureObj, UnsafeFutureObj};

// Extension traits and combinators
#[allow(clippy::module_inception)]
mod future;
#[cfg(feature = "std")]
pub use self::future::CatchUnwind;
#[deprecated(note = "This is now an alias for [Flatten](Flatten)")]
pub use self::future::FlattenStream;
pub use self::future::{
    Flatten, Fuse, FutureExt, Inspect, IntoStream, Map, MapInto, NeverError, Then, UnitError,
};
#[cfg(feature = "channel")]
#[cfg_attr(docsrs, doc(cfg(feature = "channel")))]
#[cfg(feature = "std")]
pub use self::future::{Remote, RemoteCompletionHandle, RemoteHandle};
#[cfg(any(feature = "std", all(feature = "alloc", feature = "spin")))]
pub use self::future::{Shared, WeakShared};

mod try_future;
#[cfg(feature = "sink")]
#[cfg_attr(docsrs, doc(cfg(feature = "sink")))]
pub use self::try_future::FlattenSink;
pub use self::try_future::{
    AndThen, ErrInto, InspectErr, InspectOk, IntoFuture, MapErr, MapOk, MapOkOrElse, OkInto,
    OrElse, TryFlatten, TryFlattenStream, TryFutureExt, UnwrapOrElse,
};

// Primitive futures

mod lazy;
pub use self::lazy::{Lazy, lazy};

mod pending;
pub use self::pending::{Pending, pending};

mod maybe_done;
pub use self::maybe_done::{MaybeDone, maybe_done};

mod try_maybe_done;
pub use self::try_maybe_done::{TryMaybeDone, try_maybe_done};

mod option;
pub use self::option::OptionFuture;

mod poll_fn;
pub use self::poll_fn::{PollFn, poll_fn};

mod poll_immediate;
pub use self::poll_immediate::{PollImmediate, poll_immediate};

mod ready;
pub use self::ready::{Ready, err, ok, ready};

mod always_ready;
pub use self::always_ready::{AlwaysReady, always_ready};

mod join;
pub use self::join::{Join, join};

#[cfg(feature = "alloc")]
mod join_all;
#[cfg(feature = "alloc")]
pub use self::join_all::{JoinAll, join_all};

mod select;
pub use self::select::{Select, select};

#[cfg(feature = "alloc")]
mod select_all;
#[cfg(feature = "alloc")]
pub use self::select_all::{SelectAll, select_all};

mod try_join;
pub use self::try_join::{TryJoin, try_join};

#[cfg(feature = "alloc")]
mod try_join_all;
#[cfg(feature = "alloc")]
pub use self::try_join_all::{TryJoinAll, try_join_all};

mod try_select;
pub use self::try_select::{TrySelect, try_select};

#[cfg(feature = "alloc")]
mod select_ok;
#[cfg(feature = "alloc")]
pub use self::select_ok::{SelectOk, select_ok};

mod either;
pub use self::either::Either;

#[cfg(target_has_atomic = "ptr")]
#[cfg(feature = "alloc")]
mod abortable;
#[cfg(target_has_atomic = "ptr")]
#[cfg(feature = "alloc")]
pub use self::abortable::abortable;
#[cfg(target_has_atomic = "ptr")]
#[cfg(feature = "alloc")]
pub use crate::abortable::{AbortHandle, AbortRegistration, Abortable, Aborted};

// Just a helper function to ensure the futures we're returning all have the
// right implementations.
pub(crate) fn assert_future<T, F>(future: F) -> F
where
    F: Future<Output = T>,
{
    future
}
