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

use core::pin::Pin;
use futures_core::task::{Poll, Context};
#[cfg(feature = "alloc")]
use alloc::boxed::Box;

pub use futures_core::stream::{FusedStream, Stream};

/// An owned dynamically typed [`Stream`] for use in cases where you can't
/// statically type your result or need to add some indirection.
#[cfg(feature = "alloc")]
pub type BoxStream<'a, T> = Pin<Box<dyn Stream<Item = T> + Send + 'a>>;

/// `BoxStream`, but without the `Send` requirement.
#[cfg(feature = "alloc")]
pub type LocalBoxStream<'a, T> = Pin<Box<dyn Stream<Item = T> + 'a>>;

mod private_try_stream {
    use super::Stream;

    pub trait Sealed {}

    impl<S, T, E> Sealed for S where S: ?Sized + Stream<Item = Result<T, E>> {}
}

/// A convenience for streams that return `Result` values that includes
/// a variety of adapters tailored to such futures.
pub trait TryStream: Stream + private_try_stream::Sealed {
    /// The type of successful values yielded by this future
    type Ok;

    /// The type of failures yielded by this future
    type Error;

    /// Poll this `TryStream` as if it were a `Stream`.
    ///
    /// This method is a stopgap for a compiler limitation that prevents us from
    /// directly inheriting from the `Stream` trait; in the future it won't be
    /// needed.
    fn try_poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>)
        -> Poll<Option<Result<Self::Ok, Self::Error>>>;
}

impl<S, T, E> TryStream for S
    where S: ?Sized + Stream<Item = Result<T, E>>
{
    type Ok = T;
    type Error = E;

    fn try_poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>)
        -> Poll<Option<Result<Self::Ok, Self::Error>>>
    {
        self.poll_next(cx)
    }
}

// Extension traits and combinators

#[allow(clippy::module_inception)]
mod stream;
pub use self::stream::{
    Chain, Collect, Concat, Cycle, Enumerate, Filter, FilterMap, FlatMap, Flatten, Fold, ForEach,
    Fuse, Inspect, Map, Next, Peek, Peekable, Scan, SelectNextSome, Skip, SkipWhile, StreamExt,
    StreamFuture, Take, TakeUntil, TakeWhile, Then, Unzip, Zip,
};

#[cfg(feature = "std")]
pub use self::stream::CatchUnwind;

#[cfg(feature = "alloc")]
pub use self::stream::Chunks;

#[cfg(feature = "alloc")]
pub use self::stream::ReadyChunks;

#[cfg(feature = "sink")]
#[cfg_attr(docsrs, doc(cfg(feature = "sink")))]
pub use self::stream::Forward;

#[cfg_attr(feature = "cfg-target-has-atomic", cfg(target_has_atomic = "ptr"))]
#[cfg(feature = "alloc")]
pub use self::stream::{BufferUnordered, Buffered, ForEachConcurrent};

#[cfg_attr(feature = "cfg-target-has-atomic", cfg(target_has_atomic = "ptr"))]
#[cfg(feature = "sink")]
#[cfg_attr(docsrs, doc(cfg(feature = "sink")))]
#[cfg(feature = "alloc")]
pub use self::stream::{ReuniteError, SplitSink, SplitStream};

mod try_stream;
pub use self::try_stream::{
    try_unfold, AndThen, ErrInto, InspectErr, InspectOk, IntoStream, MapErr, MapOk, OrElse,
    TryCollect, TryConcat, TryFilter, TryFilterMap, TryFlatten, TryFold, TryForEach, TryNext,
    TrySkipWhile, TryStreamExt, TryTakeWhile, TryUnfold,
};

#[cfg(feature = "io")]
#[cfg_attr(docsrs, doc(cfg(feature = "io")))]
#[cfg(feature = "std")]
pub use self::try_stream::IntoAsyncRead;

#[cfg_attr(feature = "cfg-target-has-atomic", cfg(target_has_atomic = "ptr"))]
#[cfg(feature = "alloc")]
pub use self::try_stream::{TryBufferUnordered, TryBuffered, TryForEachConcurrent};

// Primitive streams

mod iter;
pub use self::iter::{iter, Iter};

mod repeat;
pub use self::repeat::{repeat, Repeat};

mod repeat_with;
pub use self::repeat_with::{repeat_with, RepeatWith};

mod empty;
pub use self::empty::{empty, Empty};

mod once;
pub use self::once::{once, Once};

mod pending;
pub use self::pending::{pending, Pending};

mod poll_fn;
pub use self::poll_fn::{poll_fn, PollFn};

mod select;
pub use self::select::{select, Select};

mod unfold;
pub use self::unfold::{unfold, Unfold};

cfg_target_has_atomic! {
    #[cfg(feature = "alloc")]
    mod futures_ordered;
    #[cfg(feature = "alloc")]
    pub use self::futures_ordered::FuturesOrdered;

    #[cfg(feature = "alloc")]
    pub mod futures_unordered;
    #[cfg(feature = "alloc")]
    #[doc(inline)]
    pub use self::futures_unordered::FuturesUnordered;

    #[cfg(feature = "alloc")]
    mod select_all;
    #[cfg(feature = "alloc")]
    pub use self::select_all::{select_all, SelectAll};
}

// Just a helper function to ensure the streams we're returning all have the
// right implementations.
pub(crate) fn assert_stream<T, S>(stream: S) -> S
where
    S: Stream<Item = T>,
{
    stream
}
