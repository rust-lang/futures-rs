# futures-rs

This library is an implementation of **zero cost futures** in Rust.

[![Build Status](https://travis-ci.org/alexcrichton/futures-rs.svg?branch=master)](https://travis-ci.org/alexcrichton/futures-rs)
[![Build status](https://ci.appveyor.com/api/projects/status/yl5w3ittk4kggfsh?svg=true)](https://ci.appveyor.com/project/alexcrichton/futures-rs)
[![Coverage Status](https://coveralls.io/repos/github/alexcrichton/futures-rs/badge.svg?branch=master)](https://coveralls.io/github/alexcrichton/futures-rs?branch=master)

[Documentation](http://alexcrichton.com/futures-rs)

## Usage

First, add this to your `Cargo.toml`:

```toml
[dependencies]
futures = { git = "https://github.com/alexcrichton/futures-rs" }
```

Next, add this to your crate:

```rust
extern crate futures;

use futures::Future;
```

And then, use futures!

## What's in this project

This repository currently houses a number of futures-related crates, all with
associated documentation:

* [`futures`] - the core abstraction of zero-cost futures
* [`futures-io`] - core zero-cost I/O abstractions expressed with futures and
                   streams
* [`futures-mio`] - a concrete implementation of TCP/UDP `futures-io`
                    abstractions backed by `mio`
* [`futures-socks5`] - an implementation of an efficient SOCKSv5 proxy server
                       showcasing `futures`, `futures-io`, and `futures-mio`
* [`futures-tls`] - TLS/SSL streams built on `futures-io` abstractions
                    implementing both client and server side connections, with
                    support for the native system library on all platforms
* [`futures-cpupool`] - a thread pool for compute-bound work in event loops
* [`futures-minihttp`] - a simple HTTP server with some "hello world" examples
                         that show the screaming fast performance of the futures
                         and mio stack
* [`tls-example`] - an HTTP server, built with minihttp, which accepts TLS
                    connections, currently this demo requires OpenSSL
* [`futures-curl`] - an asynchronous HTTP client backed by libcurl exposed
                     through futures

[`futures`]: http://alexcrichton.com/futures-rs/futures
[`futures-io`]: http://alexcrichton.com/futures-rs/futures_io
[`futures-mio`]: http://alexcrichton.com/futures-rs/futures_mio
[`futures-tls`]: http://alexcrichton.com/futures-rs/futures_tls
[`futures-curl`]: http://alexcrichton.com/futures-rs/futures_curl
[`futures-cpupool`]: http://alexcrichton.com/futures-rs/futures_cpupool
[`futures-minihttp`]: https://github.com/alexcrichton/futures-rs/tree/master/futures-minihttp
[`futures-socks5`]: https://github.com/alexcrichton/futures-rs/blob/master/futures-socks5/src/main.rs
[`tls-example`]: https://github.com/alexcrichton/futures-rs/tree/master/futures-minihttp/tls-example

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
style, is an interface for **zero allocation futures**. Like iterators in Rust
the are a wide variety of helpful combinators associated with this trait which
are also zero allocation and serve as a succinct and powerful way to compose
computations on futures.

[Future]: http://alexcrichton.com/futures-rs/futures/trait.Future.html

The `Future` trait is driven by two methods, [`poll`][poll] and
[`schedule`][schedule], which allow pulling values out of a future and otherwise
getting notified when a future is complete. More documentation can be found on
the associated methods themselves.

[poll]: http://alexcrichton.com/futures-rs/futures/trait.Future.html#tymethod.poll
[schedule]: http://alexcrichton.com/futures-rs/futures/trait.Future.html#tymethod.schedule

## I/O with futures

With the power of zero-allocation futures we can take futures all the way down
the stack to the I/O layer. The [`futures-io` crate][futures-io] provides an
abstraction for I/O objects as a stream of readiness notifications plus `Read`
and `Write` traits along with a number of combinators you'd expect when working
with `std::io`.

These abstractions can be implemented by the [`futures-mio` crate][futures-mio]
to use [`mio`][mio] to power I/O. Finally we can then use these abstractions to
build a [`futures-tls` crate][futures-tls] crate to provide TLS/SSL streams over
arbitrary read/write streams.

[futures-io]: http://alexcrichton.com/futures-rs/futures_io/index.html
[futures-mio]: http://alexcrichton.com/futures-rs/futures_mio/index.html
[futures-tls]: http://alexcrichton.com/futures-rs/futures_tls/index.html
[mio]: https://github.com/carllerche/mio

# License

`futures-rs` is primarily distributed under the terms of both the MIT license and
the Apache License (Version 2.0), with portions covered by various BSD-like
licenses.

See LICENSE-APACHE, and LICENSE-MIT for details.
