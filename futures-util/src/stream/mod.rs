//! Asynchronous streams.
//!
//! This module contains:
//!
//! - The [`Stream`] trait, for objects that can asynchronously produce a
//!   sequence of values.
//! - The [`StreamExt`] and [`TryStreamExt`] trait, which provides adapters for
//!   chaining and composing streams.
//! - Top-level stream constructors like [`iter`](iter()) which creates a
//!   stream from an iterator.

#[cfg(feature = "alloc")]
pub use futures_core::stream::{BoxStream, LocalBoxStream};
pub use futures_core::stream::{FusedStream, Stream, TryStream};

// Extension traits and combinators

#[allow(clippy::module_inception)]
mod stream;
#[cfg(feature = "std")]
pub use self::stream::CatchUnwind;
#[cfg(feature = "alloc")]
pub use self::stream::Chunks;
#[cfg(feature = "sink")]
#[cfg_attr(docsrs, doc(cfg(feature = "sink")))]
pub use self::stream::Forward;
#[cfg(feature = "alloc")]
pub use self::stream::ReadyChunks;
pub use self::stream::{
    All, Any, Chain, Collect, Concat, Count, Cycle, Enumerate, Filter, FilterMap, FlatMap, Flatten,
    Fold, ForEach, Fuse, Inspect, Map, Next, NextIf, NextIfEq, Peek, PeekMut, Peekable, Scan,
    SelectNextSome, Skip, SkipWhile, StreamExt, StreamFuture, Take, TakeUntil, TakeWhile, Then,
    TryFold, TryForEach, Unzip, Zip,
};
#[cfg(target_has_atomic = "ptr")]
#[cfg(feature = "alloc")]
pub use self::stream::{
    BufferUnordered, Buffered, FlatMapUnordered, FlattenUnordered, ForEachConcurrent,
    TryForEachConcurrent,
};
#[cfg(target_has_atomic = "ptr")]
#[cfg(feature = "sink")]
#[cfg_attr(docsrs, doc(cfg(feature = "sink")))]
#[cfg(feature = "alloc")]
pub use self::stream::{ReuniteError, SplitSink, SplitStream};

mod try_stream;
#[cfg(feature = "io")]
#[cfg_attr(docsrs, doc(cfg(feature = "io")))]
#[cfg(feature = "std")]
pub use self::try_stream::IntoAsyncRead;
#[cfg(feature = "sink")]
#[cfg_attr(docsrs, doc(cfg(feature = "sink")))]
pub use self::try_stream::TryForward;
pub use self::try_stream::{
    AndThen, ErrInto, InspectErr, InspectOk, MapErr, MapOk, OrElse, TryAll, TryAny, TryCollect,
    TryConcat, TryFilter, TryFilterMap, TryFlatten, TryNext, TrySkipWhile, TryStreamExt,
    TryTakeWhile, TryUnfold, try_unfold,
};
#[cfg(target_has_atomic = "ptr")]
#[cfg(feature = "alloc")]
pub use self::try_stream::{TryBufferUnordered, TryBuffered, TryFlattenUnordered};
#[cfg(feature = "alloc")]
pub use self::try_stream::{TryChunks, TryChunksError, TryReadyChunks, TryReadyChunksError};

// Primitive streams

mod iter;
pub use self::iter::{Iter, iter};

mod repeat;
pub use self::repeat::{Repeat, repeat};

mod repeat_with;
pub use self::repeat_with::{RepeatWith, repeat_with};

mod empty;
pub use self::empty::{Empty, empty};

mod once;
pub use self::once::{Once, once};

mod pending;
pub use self::pending::{Pending, pending};

mod poll_fn;
pub use self::poll_fn::{PollFn, poll_fn};

mod poll_immediate;
pub use self::poll_immediate::{PollImmediate, poll_immediate};

mod select;
pub use self::select::{Select, select};

mod select_with_strategy;
pub use self::select_with_strategy::{PollNext, SelectWithStrategy, select_with_strategy};

mod unfold;
pub use self::unfold::{Unfold, unfold};

#[cfg(target_has_atomic = "ptr")]
#[cfg(feature = "alloc")]
mod futures_ordered;
#[cfg(target_has_atomic = "ptr")]
#[cfg(feature = "alloc")]
pub use self::futures_ordered::FuturesOrdered;

#[cfg(any(target_has_atomic = "ptr", feature = "portable-atomic-alloc"))]
#[cfg(feature = "alloc")]
pub mod futures_unordered;
#[cfg(any(target_has_atomic = "ptr", feature = "portable-atomic-alloc"))]
#[cfg(feature = "alloc")]
#[doc(inline)]
pub use self::futures_unordered::FuturesUnordered;

#[cfg(target_has_atomic = "ptr")]
#[cfg(feature = "alloc")]
pub mod select_all;
#[cfg(target_has_atomic = "ptr")]
#[cfg(feature = "alloc")]
#[doc(inline)]
pub use self::select_all::{SelectAll, select_all};

#[cfg(target_has_atomic = "ptr")]
#[cfg(feature = "alloc")]
mod abortable;
#[cfg(target_has_atomic = "ptr")]
#[cfg(feature = "alloc")]
pub use self::abortable::abortable;
#[cfg(target_has_atomic = "ptr")]
#[cfg(feature = "alloc")]
pub use crate::abortable::{AbortHandle, AbortRegistration, Abortable, Aborted};

// Just a helper function to ensure the streams we're returning all have the
// right implementations.
pub(crate) fn assert_stream<T, S>(stream: S) -> S
where
    S: Stream<Item = T>,
{
    stream
}
