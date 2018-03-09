//! Abstractions for asynchronous programming.
//!
//! This crate provides a number of core abstractions for writing asynchronous code:
//!
//! - [Futures](::Future) (sometimes called promises), which represent a single
//! asychronous computation that may result in a final value or an error.
//!
//! - [Streams](::Stream), which represent a series of values or errors produced asynchronously.
//!
//! - [Sinks](::Sink), which support asynchronous writing of data.
//!
//! - [Executors](::executor), which are responsible for running asynchronous tasks.
//!
//! The crate also contains abstractions for [asynchronous I/O](::io) and
//! [cross-task communication](::channel).
//!
//! Underlying all of this is the *task system*, which is a form of lightweight
//! threading. Large asynchronous computations are built up using futures,
//! streams and sinks, and then spawned as independent tasks that are run to
//! completion, but *do not block* the thread running them.

#![no_std]
#![doc(html_root_url = "https://docs.rs/futures/0.2.0-alpha")]

extern crate futures_core;
extern crate futures_channel;
extern crate futures_executor;
extern crate futures_io;
extern crate futures_sink;
extern crate futures_stable;
extern crate futures_util;

pub use futures_core::future::{Future, IntoFuture};
pub use futures_util::future::FutureExt;
pub use futures_core::stream::Stream;
pub use futures_util::stream::StreamExt;
pub use futures_sink::Sink;
pub use futures_util::sink::SinkExt;

// Macros redefined here because macro re-exports are unstable.

/// A macro for extracting the successful type of a `Poll<T, E>`.
///
/// This macro bakes in propagation of both errors and `Pending` signals by
/// returning early.
#[macro_export]
macro_rules! try_ready {
    ($e:expr) => (match $e {
        Ok($crate::prelude::Async::Ready(t)) => t,
        Ok($crate::prelude::Async::Pending) => return Ok($crate::prelude::Async::Pending),
        Err(e) => return Err(From::from(e)),
    })
}

/// A macro to create a `static` of type `LocalKey`.
///
/// This macro is intentionally similar to the `thread_local!`, and creates a
/// `static` which has a `get_mut` method to access the data on a task.
///
/// The data associated with each task local is per-task, so different tasks
/// will contain different values.
#[macro_export]
macro_rules! task_local {
    (static $NAME:ident: $t:ty = $e:expr) => (
        static $NAME: $crate::task::LocalKey<$t> = {
            fn __init() -> $t { $e }
            fn __key() -> ::std::any::TypeId {
                struct __A;
                ::std::any::TypeId::of::<__A>()
            }
            $crate::task::LocalKey {
                __init: __init,
                __key: __key,
            }
        };
    )
}

pub use futures_core::{Async, Poll, Never};

#[cfg(feature = "std")]
pub mod channel {
    //! Cross-task communication.
    //!
    //! Like threads, concurrent tasks sometimes need to communicate with each
    //! other. This module contains two basic abstractions for doing so:
    //!
    //! - [oneshot](::channel::oneshot), a way of sending a single value from
    //! one task to another.
    //!
    //! - [mpsc](::channel::mpsc), a multi-producer, single-consumer channel for
    //! sending values between tasks, analogous to the similarly-named structure
    //! in the standard library.

    pub use futures_channel::{oneshot, mpsc};
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
    //! pool](::executor::ThreadPool). A small set of worker threads can handle
    //! a very large set of spawned tasks (which are much lighter weight than
    //! threads).
    //!
    //! The simplest way to use a thread pool is to
    //! [`run`](::executor::ThreadPool::run) an initial task on it, which can
    //! then spawn further tasks back onto the pool to complete its work:
    //!
    //! ```
    //! use futures::executor::ThreadPool;
    //! # use futures::future::{Future, lazy};
    //! # let my_app: Box<Future<Item = (), Error = ()>> = Box::new(lazy(|| Ok(())));
    //!
    //! // assumping `my_app: Future`
    //! ThreadPool::new().run(my_app);
    //! ```
    //!
    //! The call to [`run`](::executor::ThreadPool::run) will block the current
    //! thread until the future defined by `my_app` completes, and will return
    //! the result of that future.
    //!
    //! # Spawning additional tasks
    //!
    //! There are two ways to spawn a task:
    //!
    //! - Spawn onto a "default" execuctor by calling the top-level
    //! [`spawn`](::executor::spawn) function or [pulling the executor from the
    //! task context](::task::Context::executor).
    //!
    //! - Spawn onto a specific executor by calling its
    //! [`spawn`](::executor::Executor::spawn) method directly.
    //!
    //! Every task always has an associated default executor, which is usually
    //! the executor on which the task is running.
    //!
    //! # Single-threaded execution
    //!
    //! In addition to thread pools, it's possible to run a task (and the tasks
    //! it spawns) entirely within a single thread via the
    //! [`LocalPool`](::executor::LocalPool) executor. Aside from cutting down
    //! on synchronization costs, this executor also makes it possible to
    //! execute non-`Send` tasks, via
    //! [`spawn_local`](::executor::LocalExecutor::spawn_local). The `LocalPool`
    //! is best suited for running I/O-bound tasks that do relatively little
    //! work between I/O operations.
    //!
    //! There is also a convenience function,
    //! [`block_on`](::executor::block_on), for simply running a future to
    //! completion on the current thread. However, any tasks spawned by the
    //! future will *also* be run on the current thread (via `LocalPool`), and
    //! will be dropped when the primary future completes.
    // TODO: add docs (or link to apr) for implementing an executor

    pub use futures_executor::{
        Enter, EnterError, LocalExecutor, LocalPool, Spawn, SpawnWithHandle, ThreadPool,
        ThreadPoolBuilder, block_on, enter, spawn, spawn_with_handle
    };
    pub use futures_core::executor::{SpawnError, Executor};
}

pub mod future {
    //! Asynchronous values.
    //!
    //! This module contains:
    //!
    //! - The [`Future` trait](::Future).
    //!
    //! - The [`FutureExt`](::future::FutureExt) trait, which provides adapters
    //! for chaining and composing futures.
    //!
    //! - Top-level future combinators like [`lazy`](::future::lazy) which
    //! creates a future from a closure that defines its return value, and
    //! [`result`](::future::result), which constructs a future with an
    //! immediate defined value.

    pub use futures_core::future::{
        FutureOption, FutureResult, Future, FutureFrom, IntoFuture, err, ok, result
    };
    pub use futures_util::future::{
        AndThen, Empty, Flatten, FlattenStream, ErrInto, Fuse,
        Inspect, IntoStream, Join, Join3, Join4, Join5, Lazy, LoopFn,
        Map, MapErr, OrElse, PollFn, Select, Then, Either, Loop, FutureExt, empty,
        lazy, loop_fn, poll_fn
    };

    #[cfg(feature = "std")]
    pub use futures_util::future::{
        CatchUnwind, JoinAll, SelectAll, SelectOk, Shared, SharedError, SharedItem,
        join_all, select_all, select_ok
    };
}

#[cfg(feature = "std")]
pub mod io {
    //! Asynchronous I/O.
    //!
    //! This module is the asynchronous version of `std::io`. It defines two
    //! traits, [`AsyncRead`](::io::AsyncRead) and
    //! [`AsyncWrite`](::io::AsyncWrite), which mirror the `Read` and `Write`
    //! traits of the standard library. However, these traits integrate with the
    //! asynchronous task system, so that if an I/O object isn't ready for
    //! reading (or writing), the thread is not blocked, and instead the current
    //! task is queued to be woken when I/O is ready.
    //!
    //! In addition, the [`AsyncReadExt`](::io::AsyncReadExt) and
    //! [`AsyncWriteExt`](::io::AsyncWriteExt) extension traits offer a variety
    //! of useful combinators for operating with asynchronous I/O objects,
    //! including ways to work with them using futures, streams and sinks.

    pub use futures_io::{
        Error, Initializer, IoVec, ErrorKind, AsyncRead, AsyncWrite, Result
    };
    pub use futures_util::io::{
        AsyncReadExt, AsyncWriteExt, AllowStdIo, Close, CopyInto, Flush,
        Read, ReadExact, ReadHalf, ReadToEnd, Window, WriteAll, WriteHalf,
    };
}

#[cfg(feature = "std")]
pub mod never {
    //! This module contains the `Never` type.
    //!
    //! Values of this type can never be created and will never exist.
    pub use futures_core::never::*;
}

pub mod prelude {
    //! A "prelude" for crates using the `futures` crate.
    //!
    //! This prelude is similar to the standard library's prelude in that you'll
    //! almost always want to import its entire contents, but unlike the standard
    //! library's prelude you'll have to do so manually:
    //!
    //! ```
    //! use futures::prelude::*;
    //! ```
    //!
    //! The prelude may grow over time as additional items see ubiquitous use.

    pub use futures_core::{
        Future,
        IntoFuture,
        Stream,
        Async,
        Poll,
        Never,
        task,
    };

    #[cfg(feature = "std")]
    pub use futures_core::executor::Executor;

    pub use futures_sink::Sink;

    #[cfg(feature = "std")]
    pub use futures_io::{
        AsyncRead,
        AsyncWrite,
    };

    pub use futures_util::{
        FutureExt,
        StreamExt,
        SinkExt,
    };

    #[cfg(feature = "std")]
    pub use futures_util::{
        AsyncReadExt,
        AsyncWriteExt,
    };
}

pub mod sink {
    //! Asynchronous sinks.
    //!
    //! This module contains:
    //!
    //! - The [`Sink` trait](::Sink), which allows you to asynchronously write data.
    //!
    //! - The [`SinkExt`](::sink::SinkExt) trait, which provides adapters
    //! for chaining and composing sinks.

    pub use futures_sink::Sink;

    pub use futures_util::sink::{
        Close, Fanout, Flush, Send, SendAll, SinkErrInto, SinkMapErr, With,
        WithFlatMap, SinkExt, close, flush,
    };

    #[cfg(feature = "std")]
    pub use futures_util::sink::Buffer;
}

pub mod stream {
    //! Asynchronous streams.
    //!
    //! This module contains:
    //!
    //! - The [`Stream` trait](::Stream), for objects that can asynchronously
    //! produce a sequence of values.
    //!
    //! - The [`StreamExt`](::future::StreamExt) trait, which provides adapters
    //! for chaining and composing streams.
    //!
    //! - Top-level stream contructors like [`iter_ok`](::stream::iter_ok) which
    //! creates a stream from an iterator, and
    //! [`futures_unordered`](::stream::futures_unordered()), which constructs a
    //! stream from a collection of futures.

    pub use futures_core::stream::Stream;

    pub use futures_util::stream::{
        AndThen, Chain, Concat, Empty, Filter, FilterMap, Flatten, Fold,
        ForEach, Forward, ErrInto, Fuse, Inspect, InspectErr, IterOk,
        IterResult, Map, MapErr, Once, OrElse, Peekable, PollFn, Repeat, Select,
        Skip, SkipWhile, StreamFuture, Take, TakeWhile, Then, Unfold, Zip,
        StreamExt, empty, iter_ok, iter_result, once, poll_fn, repeat, unfold,
    };

    #[cfg(feature = "std")]
    pub use futures_util::stream::{
        futures_unordered, BufferUnordered, Buffered, CatchUnwind, Chunks, Collect,
        FuturesUnordered, FuturesOrdered, ReuniteError, SelectAll, SplitSink, SplitStream,
        futures_ordered,
    };
}

pub mod task {
    //! Tools for working with tasks.
    //!
    //! This module contains:
    //!
    //! - [`Context`](::task::Context), which provides contextual data present
    //! for every task, including a handle for waking up the task.
    //!
    //! - [`Waker`](::task::Waker), a handle for waking up a task.
    //!
    //! - [`LocalKey`](::task::LocalKey), a key for task-local data; you should
    //! use the [`task_local` macro](../macro.task_local.html) to set up such keys.
    //!
    //! Tasks themselves are generally created by spawning a future onto [an
    //! executor](::executor). However, you can manually construct a task by
    //! creating your own `Context` instance, and polling a future with it.
    //!
    //! The remaining types and traits in the module are used for implementing
    //! executors or dealing with synchronization issues around task wakeup.

    pub use futures_core::task::{
        AtomicWaker, Context, LocalMap, Waker, UnsafeWake,
    };

    #[cfg(feature = "std")]
    pub use futures_core::task::{LocalKey, Wake};
}

#[cfg(feature = "nightly")]
pub mod stable {
    pub use futures_stable::{StableFuture, StableStream};

    #[cfg(feature = "std")]
    pub use futures_stable::{StableExecutor, block_on_stable};
}
