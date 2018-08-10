//! Abstractions for asynchronous programming.
//!
//! This crate provides a number of core abstractions for writing asynchronous
//! code:
//!
//! - [Futures](crate::future::Future) are single eventual values produced by
//!   asychronous computations. Some programming languages (e.g. JavaScript)
//!   call this concept "promise".
//! - [Streams](crate::stream::Stream) represent a series of values
//!   produced asynchronously.
//! - [Sinks](crate::sink::Sink) provide support for asynchronous writing of
//!   data.
//! - [Executors](crate::executor) are responsible for running asynchronous
//!   tasks.
//!
//! The crate also contains abstractions for [asynchronous I/O](crate::io) and
//! [cross-task communication](crate::channel).
//!
//! Underlying all of this is the *task system*, which is a form of lightweight
//! threading. Large asynchronous computations are built up using futures,
//! streams and sinks, and then spawned as independent tasks that are run to
//! completion, but *do not block* the thread running them.

#![feature(pin, arbitrary_self_types, futures_api)]

#![no_std]

#![warn(missing_docs, missing_debug_implementations)]
#![deny(bare_trait_objects)]

#![doc(html_root_url = "https://rust-lang-nursery.github.io/futures-api-docs/0.3.0-alpha.2/futures")]

#![cfg_attr(feature = "nightly", feature(cfg_target_has_atomic))]
#![cfg_attr(feature = "nightly", feature(use_extern_macros))]

#[doc(hidden)] pub use futures_util::core_reexport;

#[doc(hidden)] pub use futures_core::future::Future;
#[doc(hidden)] pub use futures_core::future::TryFuture;
#[doc(hidden)] pub use futures_util::future::FutureExt;
#[doc(hidden)] pub use futures_util::try_future::TryFutureExt;

#[doc(hidden)] pub use futures_core::stream::Stream;
#[doc(hidden)] pub use futures_core::stream::TryStream;
#[doc(hidden)] pub use futures_util::stream::StreamExt;
#[doc(hidden)] pub use futures_util::try_stream::TryStreamExt;

#[doc(hidden)] pub use futures_sink::Sink;
#[doc(hidden)] pub use futures_util::sink::SinkExt;

#[doc(hidden)] pub use futures_core::task::Poll;

// Macro reexports
pub use futures_util::{
    // Error/readiness propagation
    try_ready, try_poll, ready,
};
#[cfg(feature = "std")]
pub use futures_util::{
    // Async-await
    join, try_join, select, pending, poll, spawn, spawn_with_handle,
};

#[cfg(feature = "std")]
pub mod channel {
    //! Cross-task communication.
    //!
    //! Like threads, concurrent tasks sometimes need to communicate with each
    //! other. This module contains two basic abstractions for doing so:
    //!
    //! - [oneshot](crate::channel::oneshot), a way of sending a single value
    //!   from one task to another.
    //! - [mpsc](crate::channel::mpsc), a multi-producer, single-consumer
    //!   channel for sending values between tasks, analogous to the
    //!   similarly-named structure in the standard library.

    pub use futures_channel::{oneshot, mpsc};
}

#[cfg(feature = "compat")]
pub mod compat {
    //! Interop between `futures` 0.1 and 0.3.
    //!

    pub use futures_util::compat::{
        Compat,
        Executor01Future,
        Executor01As03,
        Executor01CompatExt,
        Future01CompatExt,
        Stream01CompatExt,
    };

    #[cfg(feature = "tokio-compat")]
    pub use futures_util::compat::TokioDefaultExecutor;
}

#[cfg(feature = "std")]
pub mod executor {
    //! Task execution.
    //!
    //! All asynchronous computation occurs within an executor, which is
    //! capable of spawning futures as tasks. This module provides several
    //! built-in executors, as well as tools for building your own.
    //!
    //! # Using a thread pool (M:N task scheduling)
    //!
    //! Most of the time tasks should be executed on a [thread
    //! pool](crate::executor::ThreadPool). A small set of worker threads can
    //! handle a very large set of spawned tasks (which are much lighter weight
    //! than threads).
    //!
    //! The simplest way to use a thread pool is to
    //! [`run`](crate::executor::ThreadPool::run) an initial task on it, which
    //! can then spawn further tasks back onto the pool to complete its work:
    //!
    //! ```
    //! #![feature(pin, arbitrary_self_types, futures_api)]
    //! use futures::executor::ThreadPool;
    //! # use futures::future::{Future, lazy};
    //! # let my_app = lazy(|_| 42);
    //!
    //! // assumping `my_app: Future`
    //! ThreadPool::new().expect("Failed to create threadpool").run(my_app);
    //! ```
    //!
    //! The call to [`run`](crate::executor::ThreadPool::run) will block the
    //! current thread until the future defined by `my_app` completes, and will
    //! return the result of that future.
    //!
    //! # Spawning additional tasks
    //!
    //! There are two ways to spawn a task:
    //!
    //! - Spawn onto a "default" execuctor by calling the top-level
    //!   [`spawn`](crate::executor::spawn) function or [pulling the executor
    //!   from the task context](crate::task::Context::executor).
    //! - Spawn onto a specific executor by calling its
    //!   [`spawn_obj`](crate::executor::Executor::spawn_obj) method directly.
    //!
    //! Every task always has an associated default executor, which is usually
    //! the executor on which the task is running.
    //!
    //! # Single-threaded execution
    //!
    //! In addition to thread pools, it's possible to run a task (and the tasks
    //! it spawns) entirely within a single thread via the
    //! [`LocalPool`](crate::executor::LocalPool) executor. Aside from cutting
    //! down on synchronization costs, this executor also makes it possible to
    //! execute non-`Send` tasks, via
    //! [`spawn_local_obj`](crate::executor::LocalExecutor::spawn_local_obj).
    //! The `LocalPool` is best suited for running I/O-bound tasks that do
    //! relatively little work between I/O operations.
    //!
    //! There is also a convenience function,
    //! [`block_on`](crate::executor::block_on), for simply running a future to
    //! completion on the current thread, while routing any spawned tasks
    //! to a global thread pool.

    pub use futures_executor::{
        BlockingStream,
        Enter, EnterError,
        LocalExecutor, LocalPool,
        ThreadPool, ThreadPoolBuilder,
        block_on, block_on_stream, enter,
    };
}

pub mod future {
    //! Asynchronous values.
    //!
    //! This module contains:
    //!
    //! - The [`Future` trait](crate::future::Future).
    //! - The [`FutureExt`](crate::future::FutureExt) trait, which provides
    //!   adapters for chaining and composing futures.
    //! - Top-level future combinators like [`lazy`](crate::future::lazy) which
    //!   creates a future from a closure that defines its return value, and
    //!   [`ready`](crate::future::ready), which constructs a future with an
    //!   immediate defined value.

    pub use futures_core::future::{
        Future, TryFuture,
        FutureObj, LocalFutureObj, UnsafeFutureObj,
    };

    pub use futures_util::future::{
        empty, Empty,
        lazy, Lazy,
        maybe_done, MaybeDone,
        poll_fn, PollFn,
        ready, Ready,

        OptionFuture,

        FutureExt,
        FlattenStream, Flatten, Fuse, Inspect, IntoStream, Join, Join3, Join4,
        Join5, Map, Then, WithExecutor,
    };

    #[cfg(feature = "std")]
    pub use futures_util::future::{
        abortable, Abortable, AbortHandle, AbortRegistration, Aborted,
        // For FutureExt:
        CatchUnwind, Shared

        // ToDo: JoinAll, SelectAll, SelectOk, join_all, select_all, select_ok
    };

    pub use futures_util::try_future::{
        TryFutureExt,
        AndThen, ErrInto, FlattenSink, IntoFuture, MapErr, MapOk, OrElse,
        UnwrapOrElse,
        TryJoin, TryJoin3, TryJoin4, TryJoin5,
    };
}

#[cfg(feature = "std")]
pub mod io {
    //! Asynchronous I/O.
    //!
    //! This module is the asynchronous version of `std::io`. It defines two
    //! traits, [`AsyncRead`](crate::io::AsyncRead) and
    //! [`AsyncWrite`](crate::io::AsyncWrite), which mirror the `Read` and
    //! `Write` traits of the standard library. However, these traits integrate
    //! with the asynchronous task system, so that if an I/O object isn't ready
    //! for reading (or writing), the thread is not blocked, and instead the
    //! current task is queued to be woken when I/O is ready.
    //!
    //! In addition, the [`AsyncReadExt`](crate::io::AsyncReadExt) and
    //! [`AsyncWriteExt`](crate::io::AsyncWriteExt) extension traits offer a
    //! variety of useful combinators for operating with asynchronous I/O
    //! objects, including ways to work with them using futures, streams and
    //! sinks.

    pub use futures_io::{
        Error, Initializer, IoVec, ErrorKind, AsyncRead, AsyncWrite, Result
    };
    pub use futures_util::io::{
        AsyncReadExt, AsyncWriteExt, AllowStdIo, Close, CopyInto, Flush,
        Read, ReadExact, ReadHalf, ReadToEnd, Window, WriteAll, WriteHalf,
    };
}

pub mod prelude {
    //! A "prelude" for crates using the `futures` crate.
    //!
    //! This prelude is similar to the standard library's prelude in that you'll
    //! almost always want to import its entire contents, but unlike the
    //! standard library's prelude you'll have to do so manually:
    //!
    //! ```
    //! use futures::prelude::*;
    //! ```
    //!
    //! The prelude may grow over time as additional items see ubiquitous use.

    pub use crate::future::{self, Future, TryFuture, FutureExt, TryFutureExt};
    pub use crate::stream::{self, Stream, TryStream, StreamExt, TryStreamExt};
    pub use crate::task::{self, Poll, ExecutorExt};
    pub use crate::sink::{self, Sink, SinkExt};

    #[cfg(feature = "std")]
    pub use crate::io::{ AsyncRead, AsyncWrite, AsyncReadExt, AsyncWriteExt };
}

pub mod sink {
    //! Asynchronous sinks.
    //!
    //! This module contains:
    //!
    //! - The [`Sink` trait](crate::sink::Sink), which allows you to
    //!   asynchronously write data.
    //! - The [`SinkExt`](crate::sink::SinkExt) trait, which provides adapters
    //!   for chaining and composing sinks.

    pub use futures_sink::Sink;

    pub use futures_util::sink::{
        Close, Flush, Send, SendAll, SinkErrInto, SinkMapErr, With,
        SinkExt, Fanout, Drain, DrainError, drain,
        // WithFlatMap,
    };

    #[cfg(feature = "std")]
    pub use futures_util::sink::Buffer;
}

pub mod stream {
    //! Asynchronous streams.
    //!
    //! This module contains:
    //!
    //! - The [`Stream` trait](crate::stream::Stream), for objects that can
    //!   asynchronously produce a sequence of values.
    //! - The [`StreamExt`](crate::stream::StreamExt) trait, which provides
    //!   adapters for chaining and composing streams.
    //! - Top-level stream contructors like [`iter_ok`](crate::stream::iter)
    //!   which creates a stream from an iterator, and
    //!   [`futures_unordered`](crate::stream::futures_unordered()), which
    //!   constructs a stream from a collection of futures.

    pub use futures_core::stream::{Stream, TryStream};

    pub use futures_util::stream::{
        iter, Iter,
        repeat, Repeat,
        empty, Empty,
        once, Once,
        poll_fn, PollFn,
        unfold, Unfold,

        StreamExt,
        Chain, Concat, Filter, FilterMap, Flatten, Fold, Forward, ForEach, Fuse,
        StreamFuture, Inspect, Map, Next, Peekable, Select, Skip, SkipWhile,
        Take, TakeWhile, Then, Zip
    };

    #[cfg(feature = "std")]
    pub use futures_util::stream::{
        futures_ordered, FuturesOrdered,
        futures_unordered, FuturesUnordered,

        // For StreamExt:
        BufferUnordered, Buffered, CatchUnwind, Chunks, Collect, SplitStream,
        SplitSink, ReuniteError,

        // ToDo: select_all, SelectAll,
    };

    pub use futures_util::try_stream::{
        TryStreamExt,
        TryNext, TryForEach, ErrInto,
        TryFold, TrySkipWhile,
        // ToDo: AndThen, ErrInto, InspectErr, MapErr, OrElse
    };

    #[cfg(feature = "std")]
    pub use futures_util::try_stream::{
        // For TryStreamExt:
        TryCollect, TryBufferUnordered,
        // ToDo: AndThen, InspectErr, MapErr, OrElse
    };
}

pub mod task {
    //! Tools for working with tasks.
    //!
    //! This module contains:
    //!
    //! - [`Context`](crate::task::Context), which provides contextual data
    //!   present for every task, including a handle for waking up the task.
    //! - [`Waker`](crate::task::Waker), a handle for waking up a task.
    //!
    //! Tasks themselves are generally created by spawning a future onto [an
    //! executor](crate::executor). However, you can manually construct a task
    //! by creating your own `Context` instance, and polling a future with it.
    //!
    //! The remaining types and traits in the module are used for implementing
    //! executors or dealing with synchronization issues around task wakeup.

    pub use futures_core::task::{
        Context, Poll, Executor,
        Waker, LocalWaker, UnsafeWake,
        SpawnErrorKind, SpawnObjError, SpawnLocalObjError,
    };

    #[cfg(feature = "std")]
    pub use futures_core::task::{
        Wake, local_waker, local_waker_from_nonlocal
    };

    pub use futures_util::task::{ExecutorExt, SpawnError};

    #[cfg(feature = "std")]
    pub use futures_util::task::{
        LocalWakerRef, local_waker_ref, local_waker_ref_from_nonlocal, JoinHandle
    };

    #[cfg_attr(
        feature = "nightly",
        cfg(all(target_has_atomic = "cas", target_has_atomic = "ptr"))
    )]
    pub use futures_util::task::AtomicWaker;
}
