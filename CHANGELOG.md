# 0.3.5 - 2020-05-08
* Added `StreamExt::flat_map`.
* Added `StreamExt::ready_chunks`.
* Added `*_unpin` methods to `SinkExt`.
* Added a `cancellation()` future to `oneshot::Sender`.
* Added `reunite` method to `ReadHalf` and `WriteHalf`.
* Added `Extend` implementations for `Futures(Un)Ordered` and `SelectAll`.
* Added support for reexporting the `join!` and `select!` macros.
* Added `no_std` support for the `pending!` and `poll!` macros.
* Added `Send` and `Sync` support for `AssertUnmoved`.
* Fixed a bug where `Shared` wasn't relinquishing control to the executor.
* Removed the `Send` bound on the output of `RemoteHandle`.
* Relaxed bounds on `FuturesUnordered`.
* Reorganized internal tests to work under different `--feature`s.
* Reorganized the bounds on `StreamExt::forward`.
* Removed and replaced a large amount of internal `unsafe`.

# 0.3.4 - 2020-02-06
* Fixed missing `Drop` for `UnboundedReceiver` (#2064)

# 0.3.3 - 2020-02-04
* Fixed compatibility issue with pinned facade (#2062)

# 0.3.2 - 2020-02-03
* Improved buffering performance of `SplitSink` (#1969)
* Added `select_biased!` macro (#1976)
* Added `hash_receiver` method to mpsc channel (#1962)
* Added `stream::try_unfold` (#1977)
* Fixed bug with zero-size buffers in vectored IO (#1998)
* `AtomicWaker::new()` is now `const fn` (#2007)
* Fixed bug between threadpool and user park/unparking (#2010)
* Added `stream::Peakable::peek` (#2021)
* Added `StreamExt::scan` (#2044)
* Added impl of `AsyncRead`/`Write` for `BufReader`/`Writer` (#2033)
* Added impl of `Spawn` and `LocalSpawn` for `Arc<impl Spawn` and `Rc<impl Spawn>` (#2039)
* Fixed `Sync` issues with `FuturesUnordered` (#2054)
* Added `into_inner` method for `future::Ready` (#2055)
* Added `MappedMutexGuard` API (#2056)
* Mitigated starvation issues in `FuturesUnordered` (#2049)
* Added `TryFutureExt::map_ok_or_else` (#2058)

# 0.3.1 - 2019-11-7
* Fix signature of `LocalSpawn` trait (breaking change -- see #1959)

# 0.3.0 - 2019-11-5
* Stable release along with stable async/await!
* Added async/await to default features (#1953)
* Changed `Spawn` trait and `FuturesUnordered::push` to take `&self` (#1950)
* Moved `Spawn` and `FutureObj` out of `futures-core` and into `futures-task` (#1925)
* Changed case convention for feature names (#1937)
* Added `executor` feature (#1949)
* Moved `copy_into`/`copy_buf_into` (#1948)
* Changed `SinkExt::send_all` to accept a `TryStream` (#1946)
* Removed `ThreadPool::run` (#1944)
* Changed to use our own definition of `io::Cursor` (#1943)
* Removed `BufReader::poll_seek_relative` (#1938)
* Changed `skip` to take a `usize` rather than `u64` (#1931)
* Removed `Stream` impl for `VecDeque` (#1930)
* Renamed `Peekable::peek` to `poll_peek` (#1928)
* Added immutable iterators for `FuturesUnordered` (#1922)
* Made `ThreadPool` optional (#1910)
* Renamed `oneshot::Sender::poll_cancel` to `poll_canceled` (#1908)
* Added some missing `Clone` implementations
* Documentation fixes

# 0.3.0-alpha.19 - 2019-9-25
* Stabilized the `async-await` feature (#1816)
* Made `async-await` feature no longer require `std` feature (#1815)
* Updated `proc-macro2`, `syn`, and `quote` to 1.0 (#1798)
* Exposed unstable `BiLock` (#1827)
* Renamed "nightly" feature to "unstable" (#1823)
* Moved to our own `io::{Empty, Repeat, Sink}` (#1829)
* Made `AsyncRead::initializer` API unstable (#1845)
* Moved the `Never` type from `futures-core` to `futures-util` (#1836)
* Fixed use-after-free on panic in `ArcWake::wake_by_ref` (#1797)
* Added `AsyncReadExt::chain` (#1810)
* Added `Stream::size_hint` (#1853)
* Added some missing `FusedFuture` (#1868) and `FusedStream` implementations (#1831)
* Added a `From` impl for `Mutex` (#1839)
* Added `Mutex::{get_mut, into_inner}` (#1839)
* Re-exported `TryConcat` and `TryFilter` (#1814)
* Lifted `Unpin` bound and implemented `AsyncBufRead` for `io::Take` (#1821)
* Lifted `Unpin` bounds on `get_pin_mut` (#1820)
* Changed `SendAll` to flush the `Sink` when the source `Stream` is pending (#1877)
* Set default threadpool size to one if `num_cpus::get()` returns zero (#1835)
* Removed dependency on `rand` by using our own PRNG (#1837)
* Removed `futures-core` dependency from `futures-sink` (#1832)

# 0.3.0-alpha.18 - 2019-8-9
* Rewrote `join!` and `try_join!` as procedural macros to allow passing expressions (#1783)
* Banned manual implementation of `TryFuture` and `TryStream` for forward compatibility. See #1776 for more details. (#1777)
* Changed `AsyncReadExt::read_to_end` to return the total number of bytes read (#1721)
* Changed `ArcWake::into_waker` to a free function `waker` (#1676)
* Supported trailing commas in macros (#1733)
* Removed futures-channel dependency from futures-executor (#1735)
* Supported `channel::oneshot` in no_std environment (#1749)
* Added `Future` bounds to `FusedFuture` (#1779)
* Added `Stream` bounds to `FusedStream` (#1779)
* Changed `StreamExt::boxed` to return `BoxStream` (#1780)
* Added `StreamExt::boxed_local` (#1780)
* Added `AsyncReadExt::read_to_string` (#1721)
* Implemented `AsyncWrite` for `IntoAsyncRead` (#1734)
* Added get_ref, get_mut and into_inner methods to `Compat01As03` and `Compat01As03Sink` (#1705)
* Added `ThreadPool::{spawn_ok, spawn_obj_ok}` (#1750)
* Added `TryStreamExt::try_flatten` (#1731)
* Added `FutureExt::now_or_never` (#1747)

# 0.3.0-alpha.17 - 2019-7-3
* Removed `try_ready!` macro in favor of `ready!(..)?`. (#1602)
* Removed `io::Window::{set_start, set_end}` in favor of `io::Window::set`. (#1667)
* Re-exported `pin_utils::pin_mut!` macro. (#1686)
* Made all extension traits unnamed in the prelude. (#1662)
* Allowed `?Sized` types in some methods and structs. (#1647)
* Added `Send + Sync` bounds to `ArcWake` trait to fix unsoundness. (#1654)
* Changed `AsyncReadExt::copy_into` to consume `self`. (#1674)
* Renamed `future::empty` to `pending`. (#1689)
* Added `#[must_use]` to some combinators. (#1600)
* Added `AsyncWriteExt::{write, write_vectored}`. (#1612)
* Added `AsyncReadExt::read_vectored`. (#1612)
* Added `TryFutureExt::try_poll_unpin`. (#1613)
* Added `TryFutureExt::try_flatten_stream`. (#1618)
* Added `io::BufWriter`. (#1608)
* Added `Sender::same_receiver` and `UnboundedSender::same_receiver`. (#1617)
* Added `future::try_select`. (#1622)
* Added `TryFutureExt::{inspect_ok, inspect_err}`. (#1630)
* Added `Compat::get_ref`. (#1648)
* Added `io::Window::set`. (#1667)
* Added `AsyncWriteExt::into_sink`. (#1675)
* Added `AsyncBufReadExt::copy_buf_into`. (#1674)
* Added `stream::pending`. (#1689)
* Implemented `std::error::Error` for `SpawnError`. (#1604)
* Implemented `Stream` for `FlattenSink`. (#1651)
* Implemented `Sink` for `TryFlattenStream`. (#1651)
* Implemented `AsyncRead`, `AsyncWrite`, `AsyncSeek`, `AsyncBufRead`, `FusedFuture` and  `FusedStream` for Either. (#1695)
* Replaced empty enums with `Never` type, an alias for `core::convert::Infallible`.
* Removed the `futures-channel` dependency from `futures-sink` and make `futures-sink`
  an optional dependency of `futures-channel`.
* Renamed `Sink::SinkError` to `Sink::Error`.
* Made a number of dependencies of `futures-util` optional.

# 0.3.0-alpha.16 - 2019-5-10
* Updated to new nightly `async_await`.
* Changed `AsyncRead::poll_vectored_read` and `AsyncWrite::poll_vectored_write` to use
  stabilized `std::io::{IoSlice, IoSliceMut}` instead of `iovec::IoVec`, and renamed to
  `AsyncRead::poll_read_vectored` and `AsyncWrite::poll_write_vectored`.
* Added `LocalBoxFuture` and `FutureExt::boxed_local`.
* Added `TryStreamExt::{try_filter, inspect_ok, inspect_err}`.
* Added `try_future::select_ok`.
* Added `AsyncBufReadExt::{read_line, lines}`.
* Added `io::BufReader`.

# 0.3.0-alpha.15 - 2019-4-26
* Updated to stabilized `futures_api`.
* Removed `StreamObj`, cautioned against usage of `FutureObj`.
* Changed `StreamExt::select` to a function.
* Added `AsyncBufRead` and `AsyncSeek` traits.
* Expanded trait impls to include more pinned pointers and ?Sized types.
* Added `future::Fuse::terminated` constructor.
* Added `never_error` combinator.
* Added `StreamExt::enumerate`.
* Re-added `TryStreamExt::{and_then, or_else}`.
* Added functions to partially progress a local pool.
* Changed to use our own `Either` type rather than the one from the `either` crate.

# 0.3.0-alpha.14 - 2019-4-15
* Updated to new nightly `futures_api`.
* Changed `Forward` combinator to drop sink after completion, and allow `!Unpin` `Sink`s.
* Added 0.1 <-> 0.3 compatability shim for `Sink`s.
* Changed `Sink::Item` to a generic parameter `Sink<Item>`, allowing `Sink`s to accept
  multiple different types, including types containing references.
* Changed `AsyncRead` and `AsyncWrite` to take `Pin<&mut Self>` rather than `&mut self`.
* Added support for `no_std` + `alloc` use.
* Changed `join` and `try_join` combinators to functions.
* Fixed propagation of `cfg-target-has-atomic` feature.

# 0.3.0-alpha.13 - 2019-2-20
* Updated to new nightly with stabilization candidate API.
* Removed `LocalWaker`.
* Added `#[must_use]` to `Stream` and `Sink` traits.
* Enabled using `!Unpin` futures in `JoinAll`.
* Added the `try_join_all` combinator.
* Stopped closing a whole channel upon closing of one sender.
* Removed `TokioDefaultSpawner` and `tokio-compat`.
* Moved intra-crate dependencies to exact versions.

# 0.3.0-alpha.12 - 2019-1-14
* Updated to new nightly with a modification to `Pin::set`.
* Expose `AssertUnmoved` and `PendingOnce`.
* Prevent double-panic in `AssertUnmoved`.
* Support nested invocations of the `select!` macro.
* Implement `Default` for `Mutex` and `SelectAll`.

# 0.3.0-alpha.11 - 2018-12-27
* Updated to newly stabilized versions of the `pin` and `arbitrary_self_types` features.
* Re-added `select_all` for streams.
* Added `TryStream::into_async_read` for converting from a stream of bytes into
  an `AsyncRead`.
* Added `try_poll_next_unpin`.
* Rewrote `select!` as a procedural macro for better error messages
* Exposed `join_all` from the facade

# 0.3.0-alpha.10 - 2018-11-27
* Revamped `select!` macro
* Added `select_next_some` method for getting only the `Some` elements of a stream from `select!`
* Added `futures::lock::Mutex` for async-aware synchronization.
* Fixed bug converting `Pin<Box<_>>` to `StreamObj`
* Improved performance of futures::channel
* Improved performance and documentation of `Shared`
* Add `new` method and more `derive`s to the `Compat` type
* Enabled spawning on a borrowed threadpool
* Re-added `join_all`
* Added `try_concat`

# 0.3.0-alpha.9 - 2018-10-18
* Fixed in response to new nightly handling of 2018 edition + `#![no_std]`

# 0.3.0-alpha.8 - 2018-10-16
* Fixed stack overflow in 0.1 compatibility layer
* Added AsyncRead / AsyncWrite compatibility layer
* Added Spawn -> 0.1 Executor compatibility
* Made 0.1 futures usable on 0.3 executors without an additional global `Task`, accomplished by wrapping 0.1 futures in an 0.1 `Spawn` when using them as 0.3 futures.
* Cleanups and improvments to the `AtomicWaker` implementation.

# 0.3.0-alpha.7 - 2018-10-01
* Update to new nightly which removes `Spawn` from `task::Context` and replaces `Context` with `LocalWaker`.
* Add `Spawn` and `LocalSpawn` traits and `FutureObj` and `LocalFutureObj` types to `futures-core`.

# 0.3.0-alpha.6 - 2018-09-10
* Replace usage of `crate` visibility with `pub(crate)` now that `crate` visibility is no longer included in the 2018 edition
* Remove newly-stabilized "edition" feature in Cargo.toml files

# 0.3.0-alpha.5 - 2018-09-03
* Revert usage of cargo crate renaming feature

# 0.3.0-alpha.4 - 2018-09-02
**Note: This release does not work, use `0.3.0-alpha.5` instead**

* `future::ok` and `future:err` to create result wrapping futures (similar to `future::ready`)
* `futures-test` crate with testing utilities
* `StreamExt::boxed` combinator
* Unsoundness fix for `FuturesUnordered`
* `StreamObj` (similar to `FutureObj`)
* Code examples for compatiblity layer functions
* Use cargo create renaming feature to import `futures@0.1` for compatiblily layer
* Import pinning APIs from `core::pin`
* Run Clippy in CI only when it is available

# 0.3.0-alpha.3 - 2018-08-15
* Compatibilty with newest nightly
* Futures 0.1 compatibility layer including Tokio compatibility
* Added `spawn!` and `spawn_with_handle!` macros
* Added `SpawnExt` methods `spawn` and `spawn_with_handle`
* Extracted pin macros into `pin_utils` crate
* Added `FutureExt` combinators `boxed` and `unit_error`
* Remove prelude from all doc examples (The prelude is still recommended for usage in playground examples. However, for doc examples we determined that fully expanded imports are more helpful)
* Improvements to `select!` and `join!` macros
* Added `try_join!` macro
* Added `StreamExt` combinator methods `try_join` and `for_each_concurrent`
* Added `TryStreamExt` combinator methods `into_stream`, `try_filter_map`, `try_skip_while`, `try_for_each_concurrent` and `try_buffer_unordered`
* Fix stream termination bug in `StreamExt::buffered` and `StreamExt::buffer_unordered`
* Added docs for `StreamExt::buffered`, `StreamExt::buffer_unordered`
* Added `task::local_waker_ref_from_nonlocal` and `task::local_waker_ref` functions
* CI improvements
* Doc improvements to `StreamExt::select`

# 0.3.0-alpha.2 - 2018-07-30
* The changelog is back!
* Compatiblity with futures API in latest nightly
* Code examples and doc improvements
  - IO: Methods of traits `AsyncReadExt`, `AsyncWriteExt`
  - Future:
    - Methods of trait `TryFutureExt`
    - Free functions `empty`, `lazy`, `maybe_done`, `poll_fn` and `ready`
    - Type `FutureOption`
    - Macros `join!`, `select!` and `pending!`
  - Stream: Methods of trait `TryStreamExt`
* Added `TryStreamExt` combinators `map_ok`, `map_err`, `err_into`, `try_next` and `try_for_each`
* Added `Drain`, a sink that will discard all items given to it. Can be created using the `drain` function
* Bugfix for the `write_all` combinator
* `AsyncWrite` impl for `Cursor<T: AsMut<[u8]>>`
* `FuturesUnordered` optimization: Since the context stores a `&LocalWaker` reference, it was possible to avoid cloning the `Arc` of the waker
* Futures-rs now uses Clippy
* We now use in-band lifetimes
* The `join!` and `select!` macros are now exposed by the `futures` crate
* The project logo was added to the `README.md`
* `sink::MapErr::get_pinned_mut` is now called `get_pin_mut`
* We now use the unstable `use_extern_macros` feature for macro reexports
* CI improvements: Named CI jobs, tests are now run on macOS and Linux, the docs are generated and Clippy needs to pass
* `#[deny(warnings)]` was removed from all crates and is now only enforced in the CI
* We now have a naming convention for type paramters: `Fut` future, `F` function, `St` stream, `Si` sink, `S` sink & stream, `R` reader, `W` writer, `T` value, `E` error
* "Task" is now defined as our term for "lightweight thread". The code of the executors and `FuturesUnordered` was refactored to align with this definition.

# 0.3.0-alpha.1 - 2018-07-19
* Major changes: See [the announcement](https://rust-lang-nursery.github.io/futures-rs/blog/2018/07/19/futures-0.3.0-alpha.1.html) on our new blog for details. The changes are too numerous to be covered in this changelog because nearly every line of code was modified.

# 0.1.17 - 2017-10-31

* Add a `close` method on `sink::Wait`
* Undeprecate `stream::iter` as `stream::iter_result`
* Improve performance of wait-related methods
* Tweak buffered sinks with a 0 capacity to forward directly to the underlying
  sink.
* Add `FromIterator` implementation for `FuturesOrdered` and `FuturesUnordered`.

# 0.1.16 - 2017-09-15

* A `prelude` module has been added to glob import from and pick up a whole
  bunch of useful types
* `sync::mpsc::Sender::poll_ready` has been added as an API
* `sync::mpsc::Sender::try_send` has been added as an API

# 0.1.15 - 2017-08-24

* Improve performance of `BiLock` methods
* Implement `Clone` for `FutureResult`
* Forward `Stream` trait through `SinkMapErr`
* Add `stream::futures_ordered` next to `futures_unordered`
* Reimplement `Stream::buffered` on top of `stream::futures_ordered` (much more
  efficient at scale).
* Add a `with_notify` function for abstractions which previously required
  `UnparkEvent`.
* Add `get_ref`/`get_mut`/`into_inner` functions for stream take/skip methods
* Add a `Clone` implementation for `SharedItem` and `SharedError`
* Add a `mpsc::spawn` function to spawn a `Stream` into an `Executor`
* Add a `reunite` function for `BiLock` and the split stream/sink types to
  rejoin two halves and reclaim the original item.
* Add `stream::poll_fn` to behave similarly to `future::poll_fn`
* Add `Sink::with_flat_map` like `Iterator::flat_map`
* Bump the minimum Rust version to 1.13.0
* Expose `AtomicTask` in the public API for managing synchronization around task
  notifications.
* Unify the `Canceled` type of the `sync` and `unsync` modules.
* Deprecate the `boxed` methods. These methods have caused more confusion than
  they've solved historically, so it's recommended to use a local extension
  trait or a local helper instead of the trait-based methods.
* Deprecate the `Stream::merge` method as it's less ergonomic than `select`.
* Add `oneshot::Sender::is_canceled` to test if a oneshot is canceled off a
  task.
* Deprecates `UnboundedSender::send` in favor of a method named `unbounded_send`
  to avoid a conflict with `Sink::send`.
* Deprecate the `stream::iter` function in favor of an `stream::iter_ok` adaptor
  to avoid the need to deal with `Result` manually.
* Add an `inspect` function to the `Future` and `Stream` traits along the lines
  of `Iterator::inspect`

# 0.1.14 - 2017-05-30

This is a relatively large release of the `futures` crate, although much of it
is from reworking internals rather than new APIs. The banner feature of this
release is that the `futures::{task, executor}` modules are now available in
`no_std` contexts! A large refactoring of the task system was performed in
PR #436 to accommodate custom memory allocation schemes and otherwise remove
all dependencies on `std` for the task module. More details about this change
can be found on the PR itself.

Other API additions in this release are:

* A `FuturesUnordered::push` method was added and the `FuturesUnordered` type
  itself was completely rewritten to efficiently track a large number of
  futures.
* A `Task::will_notify_current` method was added with a slightly different
  implementation than `Task::is_current` but with stronger guarantees and
  documentation wording about its purpose.
* Many combinators now have `get_ref`, `get_mut`, and `into_inner` methods for
  accessing internal futures and state.
* A `Stream::concat2` method was added which should be considered the "fixed"
  version of `concat`, this one doesn't panic on empty streams.
* An `Executor` trait has been added to represent abstracting over the concept
  of spawning a new task. Crates which only need the ability to spawn a future
  can now be generic over `Executor` rather than requiring a
  `tokio_core::reactor::Handle`.

As with all 0.1.x releases this PR is intended to be 100% backwards compatible.
All code that previously compiled should continue to do so with these changes.
As with other changes, though, there are also some updates to be aware of:

* The `task::park` function has been renamed to `task::current`.
* The `Task::unpark` function has been renamed to `Task::notify`, and in general
  terminology around "unpark" has shifted to terminology around "notify"
* The `Unpark` trait has been deprecated in favor of the `Notify` trait
  mentioned above.
* The `UnparkEvent` structure has been deprecated. It currently should perform
  the same as it used to, but it's planned that in a future 0.1.x release the
  performance will regress for crates that have not transitioned away. The
  primary primitive to replace this is the addition of a `push` function on the
  `FuturesUnordered` type. If this does not help implement your use case though,
  please let us know!
* The `Task::is_current` method is now deprecated, and you likely want to use
  `Task::will_notify_current` instead, but let us know if this doesn't suffice!

# 0.1.13 - 2017-04-05

* Add forwarding sink/stream impls for `stream::FromErr` and `sink::SinkFromErr`
* Add `PartialEq` and `Eq` to `mpsc::SendError`
* Reimplement `Shared` with `spawn` instead of `UnparkEvent`

# 0.1.12 - 2017-04-03

* Add `Stream::from_err` and `Sink::from_err`
* Allow `SendError` to be `Clone` when possible

# 0.1.11 - 2017-03-13

The major highlight of this release is the addition of a new "default" method on
the `Sink` trait, `Sink::close`. This method is used to indicate to a sink that
no new values will ever need to get pushed into it. This can be used to
implement graceful shutdown of protocols and otherwise simply indicates to a
sink that it can start freeing up resources.

Currently this method is **not** a default method to preserve backwards
compatibility, but it's intended to become a default method in the 0.2 series of
the `futures` crate. It's highly recommended to audit implementations of `Sink`
to implement the `close` method as is fit.

Other changes in this release are:

* A new select combinator, `Future::select2` was added for a heterogeneous
  select.
* A `Shared::peek` method was added to check to see if it's done.
* `Sink::map_err` was implemented
* The `log` dependency was removed
* Implementations of the `Debug` trait are now generally available.
* The `stream::IterStream` type was renamed to `stream::Iter` (with a reexport
  for the old name).
* Add a `Sink::wait` method which returns an adapter to use an arbitrary `Sink`
  synchronously.
* A `Stream::concat` method was added to concatenate a sequence of lists.
* The `oneshot::Sender::complete` method was renamed to `send` and now returns a
  `Result` indicating successful transmission of a message or not. Note that the
  `complete` method still exists, it's just deprecated.

# 0.1.10 - 2017-01-30

* Add a new `unsync` module which mirrors `sync` to the extent that it can but
  is intended to not perform cross-thread synchronization (only usable within
  one thread).
* Tweak `Shared` to work when handles may not get poll'd again.

# 0.1.9 - 2017-01-18

* Fix `Send/Sync` of a few types
* Add `future::tail_fn` for more easily writing loops
* Export SharedItem/SharedError
* Remove an unused type parameter in `from_err`

# 0.1.8 - 2017-01-11

* Fix some race conditions in the `Shared` implementation
* Add `Stream::take_while`
* Fix an unwrap in `stream::futures_unordered`
* Generalize `Stream::for_each`
* Add `Stream::chain`
* Add `stream::repeat`
* Relax `&mut self` to `&self` in `UnboundedSender::send`

# 0.1.7 - 2016-12-18

* Add a `Future::shared` method for creating a future that can be shared
  amongst threads by cloning the future itself. All derivative futures
  will resolve to the same value once the original future has been
  resolved.
* Add a `FutureFrom` trait for future-based conversion
* Fix a wakeup bug in `Receiver::close`
* Add `future::poll_fn` for quickly adapting a `Poll`-based function to
  a future.
* Add an `Either` enum with two branches to easily create one future
  type based on two different futures created on two branches of control
  flow.
* Remove the `'static` bound on `Unpark`
* Optimize `send_all` and `forward` to send as many items as possible
  before calling `poll_complete`.
* Unify the return types of the `ok`, `err`, and `result` future to
  assist returning different varieties in different branches of a function.
* Add `CpuFuture::forget` to allow the computation to continue running
  after a drop.
* Add a `stream::futures_unordered` combinator to turn a list of futures
  into a stream representing their order of completion.

# 0.1.6 - 2016-11-22

* Fix `Clone` bound on the type parameter on `UnboundedSender`

# 0.1.5 - 2016-11-22

* Fix `#![no_std]` support

# 0.1.4 - 2016-11-22

This is quite a large release relative to the previous point releases! As
with all 0.1 releases, this release should be fully compatible with the 0.1.3
release. If any incompatibilities are discovered please file an issue!

The largest changes in 0.1.4 are the addition of a `Sink` trait coupled with a
reorganization of this crate. Note that all old locations for types/traits
still exist, they're just deprecated and tagged with `#[doc(hidden)]`.

The new `Sink` trait is used to represent types which can periodically over
time accept items, but may take some time to fully process the item before
another can be accepted. Essentially, a sink is the opposite of a stream. This
trait will then be used in the tokio-core crate to implement simple framing by
modeling I/O streams as both a stream and a sink of frames.

The organization of this crate is to now have three primary submodules,
`future`, `stream`, and `sink`. The traits as well as all combinator types are
defined in these submodules. The traits and types like `Async` and `Poll` are
then reexported at the top of the crate for convenient usage. It should be a
relatively rare occasion that the modules themselves are reached into.

Finally, the 0.1.4 release comes with a new module, `sync`, in the futures
crate.  This is intended to be the home of a suite of futures-aware
synchronization primitives. Currently this is inhabited with a `oneshot` module
(the old `oneshot` function), a `mpsc` module for a new multi-producer
single-consumer channel, and a `BiLock` type which represents sharing ownership
of one value between two consumers. This module may expand over time with more
types like a mutex, rwlock, spsc channel, etc.

Notable deprecations in the 0.1.4 release that will be deleted in an eventual
0.2 release:

* The `TaskRc` type is now deprecated in favor of `BiLock` or otherwise `Arc`
  sharing.
* All future combinators should be accessed through the `future` module, not
  the top-level of the crate.
* The `Oneshot` and `Complete` types are now replaced with the `sync::oneshot`
  module.
* Some old names like `collect` are deprecated in favor of more appropriately
  named versions like `join_all`
* The `finished` constructor is now `ok`.
* The `failed` constructor is now `err`.
* The `done` constructor is now `result`.

As always, please report bugs to https://github.com/rust-lang-nursery/futures-rs and
we always love feedback! If you've got situations we don't cover, combinators
you'd like to see, or slow code, please let us know!

Full changelog:

* Improve scalability of `buffer_unordered` combinator
* Fix a memory ordering bug in oneshot
* Add a new trait, `Sink`
* Reorganize the crate into three primary modules
* Add a new `sync` module for synchronization primitives
* Add a `BiLock` sync primitive for two-way sharing
* Deprecate `TaskRc`
* Rename `collect` to `join_all`
* Use a small vec in `Events` for improved clone performance
* Add `Stream::select` for selecting items from two streams like `merge` but
  requiring the same types.
* Add `stream::unfold` constructor
* Add a `sync::mpsc` module with a futures-aware multi-producer single-consumer
  queue. Both bounded (with backpressure) and unbounded (no backpressure)
  variants are provided.
* Renamed `failed`, `finished`, and `done` combinators to `err`, `ok`, and
  `result`.
* Add `Stream::forward` to send all items to a sink, like `Sink::send_all`
* Add `Stream::split` for streams which are both sinks and streams to have
  separate ownership of the stream/sink halves
* Improve `join_all` with concurrency

# 0.1.3 - 2016-10-24

* Rewrite `oneshot` for efficiency and removing allocations on send/recv
* Errors are passed through in `Stream::take` and `Stream::skip`
* Add a `select_ok` combinator to pick the first of a list that succeeds
* Remove the unnecessary `SelectAllNext` typedef
* Add `Stream::chunks` for receiving chunks of data
* Rewrite `stream::channel` for efficiency, correctness, and removing
  allocations
* Remove `Send + 'static` bounds on the `stream::Empty` type

# 0.1.2 - 2016-10-04

* Fixed a bug in drop of `FutureSender`
* Expose the channel `SendError` type
* Add `Future::into_stream` to convert to a single-element stream
* Add `Future::flatten_to_stream` to convert a future of a stream to a stream
* impl Debug for SendError
* Add stream::once for a one element stream
* Accept IntoIterator in stream::iter
* Add `Stream::catch_unwind`

# 0.1.1 - 2016-09-09

Initial release!
