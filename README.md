# futures-rs

This library is an implementation of **zero-cost futures** in Rust.

[![Build Status](https://travis-ci.org/alexcrichton/futures-rs.svg?branch=master)](https://travis-ci.org/alexcrichton/futures-rs)
[![Build status](https://ci.appveyor.com/api/projects/status/yl5w3ittk4kggfsh?svg=true)](https://ci.appveyor.com/project/alexcrichton/futures-rs)
[![Crates.io](https://img.shields.io/crates/v/futures.svg?maxAge=2592000)](https://crates.io/crates/futures)

[Documentation](https://docs.rs/futures)

[Tutorial][tutorial]

[tutorial]: https://github.com/alexcrichton/futures-rs/blob/master/TUTORIAL.md

## Usage

First, add this to your `Cargo.toml`:

```toml
[dependencies]
futures = "0.1.6"
```

Next, add this to your crate:

```rust
extern crate futures;

use futures::Future;
```

And then, use futures! If this is your first time with futures in Rust, or at
all, check out the [tutorial].

## What's using futures?

* [`futures`] - the core abstraction of zero-cost futures
* [`futures-cpupool`] - a thread pool for compute-bound work in event loops
* [`tokio-core`] - a concrete implementation of TCP/UDP abstractions backed by
                   `mio` composable with types implementing `Future`
* [`tokio-proto`] - abstractions for easily writing new and composable protocols
                    on top of `tokio-core`
* [`tokio-socks5`] - an implementation of an efficient SOCKSv5 proxy server
                     showcasing `futures` and `tokio-core`
* [`tokio-tls`] - TLS/SSL streams built on futures, implementing both client- and
                  server-side connections, with support for the native system
                  library on all platforms
* [`tokio-curl`] - an asynchronous HTTP client backed by libcurl exposed
                   through futures
* [`tokio-uds`] - bindings for Unix domain sockets and futures
* [`tokio-minihttp`] - a simple HTTP server with some "hello world" examples
                       that show the screaming-fast performance of the futures
                       and mio stack

[`futures`]: https://docs.rs/futures
[`futures-cpupool`]: https://docs.rs/futures-cpupool
[`tokio-core`]: https://tokio-rs.github.io/tokio-core
[`tokio-proto`]: https://tokio-rs.github.io/tokio-proto
[`tokio-socks5`]: https://github.com/tokio-rs/tokio-socks5
[`tokio-tls`]: https://tokio-rs.github.io/tokio-tls
[`tokio-curl`]: https://tokio-rs.github.io/tokio-curl
[`tokio-uds`]: https://tokio-rs.github.io/tokio-uds
[`tokio-minihttp`]: https://github.com/tokio-rs/tokio-minihttp

## Why Futures?

A major missing piece of the Rust ecosystem has been how to work with
Asynchronous I/O and in general composing many I/O tasks together in a
lightweight way across libraries. Futures have worked out fantastically in other
languages to solve this problem in frameworks like [finagle] and [wangle], and
they also turn out to be a great way to solve this problem in Rust as well!

[finagle]: https://twitter.github.io/finagle/
[wangle]: https://github.com/facebook/wangle

The purpose of the `futures` library in this repository is to provide the
foundational layer to build an ecosystem of futures-generating computations so
they can all compose with one another.

## The `Future` trait

At the heart of this crate is [the `Future` trait][Future], which in true Rust
style, is an interface for **zero-allocation futures**. Like iterators in Rust,
there are a wide variety of helpful combinators associated with this trait which
are also zero-allocation and serve as a succinct and powerful way to compose
computations on futures.

[Future]: https://docs.rs/futures/^0.1/futures/future/trait.Future.html

The `Future` trait is driven by one method, [`poll`][poll], which allows
pulling values out of a future and also getting notified when a future is
complete. More documentation can be found on the associated method.

[poll]: https://docs.rs/futures/^0.1/futures/future/trait.Future.html#tymethod.pollod.poll

More information can be found in [the tutorial][tutorial-future-trait]

[tutorial-future-trait]: https://github.com/alexcrichton/futures-rs/blob/master/TUTORIAL.md#the-future-trait

## I/O with futures

With the power of zero-allocation futures we can take futures all the way down
the stack to the I/O layer. The [Tokio] project is a one-stop shop for async I/O
in Rust, starting with the [`tokio-core` crate][tokio-core] at the very bottom
layer all the way up to the [`tokio-proto` crate][tokio-proto] to enable easily
building new protocols.

[Tokio]: https://github.com/tokio-rs/tokio
[tokio-core]: https://tokio-rs.github.io/tokio-core
[tokio-proto]: https://tokio-rs.github.io/tokio-proto
[tokio-tls]: https://tokio-rs.github.io/tokio-tls
[mio]: https://github.com/carllerche/mio

# License

`futures-rs` is primarily distributed under the terms of both the MIT license and
the Apache License (Version 2.0), with portions covered by various BSD-like
licenses.

See LICENSE-APACHE, and LICENSE-MIT for details.
