//! Zero-cost Futures in Rust
//!
//! This library is an implementation of futures in Rust which aims to provide
//! a robust implementation of handling asynchronous computations, ergonomic
//! composition and usage, and zero-cost abstractions over what would otherwise
//! be written by hand.
//!
//! Futures are a concept for an object which is a proxy for another value that
//! may not be ready yet. For example issuing an HTTP request may return a
//! future for the HTTP response, as it probably hasn't arrived yet. With an
//! object representing a value that will eventually be available, futures allow
//! for powerful composition of tasks through basic combinators that can perform
//! operations like chaining computations, changing the types of futures, or
//! waiting for two futures to complete at the same time.
//!
//! You can find extensive tutorials and documentations at [https://tokio.rs]
//! for both this crate (asynchronous programming in general) as well as the
//! Tokio stack to perform async I/O with.
//!
//! [https://tokio.rs]: https://tokio.rs
//!
//! ## Installation
//!
//! Add this to your `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! futures = "0.1"
//! ```
//!
//! ## Examples
//!
//! Let's take a look at a few examples of how futures might be used:
//!
//! ```
//! extern crate futures;
//!
//! use std::io;
//! use std::time::Duration;
//! use futures::prelude::*;
//! use futures::future::Map;
//!
//! // A future is actually a trait implementation, so we can generically take a
//! // future of any integer and return back a future that will resolve to that
//! // value plus 10 more.
//! //
//! // Note here that like iterators, we're returning the `Map` combinator in
//! // the futures crate, not a boxed abstraction. This is a zero-cost
//! // construction of a future.
//! fn add_ten<F>(future: F) -> Map<F, fn(i32) -> i32>
//!     where F: Future<Item=i32>,
//! {
//!     fn add(a: i32) -> i32 { a + 10 }
//!     future.map(add)
//! }
//!
//! // Not only can we modify one future, but we can even compose them together!
//! // Here we have a function which takes two futures as input, and returns a
//! // future that will calculate the sum of their two values.
//! //
//! // Above we saw a direct return value of the `Map` combinator, but
//! // performance isn't always critical and sometimes it's more ergonomic to
//! // return a trait object like we do here. Note though that there's only one
//! // allocation here, not any for the intermediate futures.
//! fn add<'a, A, B>(a: A, b: B) -> Box<Future<Item=i32, Error=A::Error> + 'a>
//!     where A: Future<Item=i32> + 'a,
//!           B: Future<Item=i32, Error=A::Error> + 'a,
//! {
//!     Box::new(a.join(b).map(|(a, b)| a + b))
//! }
//!
//! // Futures also allow chaining computations together, starting another after
//! // the previous finishes. Here we wait for the first computation to finish,
//! // and then decide what to do depending on the result.
//! fn download_timeout(url: &str,
//!                     timeout_dur: Duration)
//!                     -> Box<Future<Item=Vec<u8>, Error=io::Error>> {
//!     use std::io;
//!     use std::net::{SocketAddr, TcpStream};
//!
//!     type IoFuture<T> = Box<Future<Item=T, Error=io::Error>>;
//!
//!     // First thing to do is we need to resolve our URL to an address. This
//!     // will likely perform a DNS lookup which may take some time.
//!     let addr = resolve(url);
//!
//!     // After we acquire the address, we next want to open up a TCP
//!     // connection.
//!     let tcp = addr.and_then(|addr| connect(&addr));
//!
//!     // After the TCP connection is established and ready to go, we're off to
//!     // the races!
//!     let data = tcp.and_then(|conn| download(conn));
//!
//!     // That all might take awhile, though, so let's not wait too long for it
//!     // to all come back. The `select` combinator here returns a future which
//!     // resolves to the first value that's ready plus the next future.
//!     //
//!     // Note we can also use the `then` combinator which is similar to
//!     // `and_then` above except that it receives the result of the
//!     // computation, not just the successful value.
//!     //
//!     // Again note that all the above calls to `and_then` and the below calls
//!     // to `map` and such require no allocations. We only ever allocate once
//!     // we hit the `Box::new()` call at the end here, which means we've built
//!     // up a relatively involved computation with only one box, and even that
//!     // was optional!
//!
//!     let data = data.map(Ok);
//!     let timeout = timeout(timeout_dur).map(Err);
//!
//!     let ret = data.select(timeout).then(|result| {
//!         match result {
//!             Ok(result) => {
//!                 let (data, _other_future) = result.split();
//!                 match data {
//!                     // One future succeeded, and it was the one which was
//!                     // downloading data from the connection.
//!                     Ok(data) => Ok(data),
//!
//!                     // The timeout fired, and otherwise no error was found, so
//!                     // we translate this to an error.
//!                     Err(_timeout) => {
//!                         Err(io::Error::new(io::ErrorKind::Other, "timeout"))
//!                     }
//!                 }
//!             }
//!
//!             // A normal I/O error happened, so we pass that on through.
//!             Err(err) => Err(err.split().0),
//!         }
//!     });
//!     return Box::new(ret);
//!
//!     fn resolve(url: &str) -> IoFuture<SocketAddr> {
//!         // ...
//! #       panic!("unimplemented");
//!     }
//!
//!     fn connect(hostname: &SocketAddr) -> IoFuture<TcpStream> {
//!         // ...
//! #       panic!("unimplemented");
//!     }
//!
//!     fn download(stream: TcpStream) -> IoFuture<Vec<u8>> {
//!         // ...
//! #       panic!("unimplemented");
//!     }
//!
//!     fn timeout(stream: Duration) -> IoFuture<()> {
//!         // ...
//! #       panic!("unimplemented");
//!     }
//! }
//! # fn main() {}
//! ```
//!
//! Some more information can also be found in the [README] for now, but
//! otherwise feel free to jump in to the docs below!
//!
//! [README]: https://github.com/alexcrichton/futures-rs#futures-rs

#![no_std]
#![doc(html_root_url = "https://docs.rs/futures/0.2")]

extern crate futures_core;
extern crate futures_channel;
extern crate futures_executor;
extern crate futures_io;
extern crate futures_sink;
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
/// This macro bakes propagation of both errors and `Pending` signals by
/// returning early.
#[macro_export]
macro_rules! try_ready {
    ($e:expr) => (match $e {
        Ok($crate::prelude::Async::Ready(t)) => t,
        Ok($crate::prelude::Async::Pending) => return Ok($crate::prelude::Async::Pending),
        Err(e) => return Err(From::from(e)),
    })
}

// TODO: task_local macro

pub use futures_core::{Async, Poll};

#[cfg(feature = "std")]
pub mod channel {
    //! Channels
    //!
    //! This module contains channels for asynchronous
    //! communication.
    pub use futures_channel::*;
}

pub mod executor {
    //! Executors
    //!
    //! This module contains the `Executor` trait, which allows futures to be
    //! spawned and executed asynchronously.

    pub use futures_executor::*;
    pub use futures_core::executor::*;
}

pub mod future {
    //! Futures
    //!
    //! This module contains the `Future` trait and adapters for this trait.

    pub use futures_core::future::*;
    pub use futures_util::future::*;
}

#[cfg(feature = "std")]
pub mod io {
    //! IO
    //!
    //! This module contains the `AsyncRead` and `AsyncWrite` traits, as well
    //! as a number of combinators and extensions for using them.
    pub use futures_io::*;
    pub use futures_util::io::*;
}

pub mod prelude {
	//! A "prelude" for crates using the `futures` crate.
	//!
	//! This prelude is similar to the standard library's prelude in that you'll
	//! almost always want to import its entire contents, but unlike the standard
	//! library's prelude you'll have to do so manually. An example of using this is:
	//!
	//! ```
	//! use futures::prelude::*;
	//! ```
	//!
	//! We may add items to this over time as they become ubiquitous as well, but
	//! otherwise this should help cut down on futures-related imports when you're
	//! working with the `futures` crate!

    pub use futures_core::{
        Future,
        IntoFuture,
        Stream,
        Async,
        Poll,
        task,
    };

    pub use futures_sink::Sink;

    pub use futures_util::{
		FutureExt,
		StreamExt,
        SinkExt,
    };

    #[cfg(feature = "std")]
    pub use futures_util::{
        AsyncRead,
        AsyncWrite,
        AsyncReadExt,
        AsyncWriteExt,
    };
}

pub mod sink {
	//! Asynchronous sinks
	//!
	//! This module contains the `Sink` trait, along with a number of adapter types
	//! for it. An overview is available in the documentation for the trait itself.
	//!
	//! You can find more information/tutorials about streams [online at
	//! https://tokio.rs][online]
	//!
	//! [online]: https://tokio.rs/docs/getting-started/streams-and-sinks/

    pub use futures_sink::*;
    pub use futures_util::sink::*;
}

pub mod stream {
	//! Asynchronous streams
	//!
	//! This module contains the `Stream` trait and a number of adaptors for this
	//! trait. This trait is very similar to the `Iterator` trait in the standard
	//! library except that it expresses the concept of blocking as well. A stream
	//! here is a sequential sequence of values which may take some amount of time
	//! in between to produce.
	//!
	//! A stream may request that it is blocked between values while the next value
	//! is calculated, and provides a way to get notified once the next value is
	//! ready as well.
	//!
	//! You can find more information/tutorials about streams [online at
	//! https://tokio.rs][online]
	//!
	//! [online]: https://tokio.rs/docs/getting-started/streams-and-sinks/

    pub use futures_core::stream::*;
    pub use futures_util::stream::*;
}

pub mod task {
    //! Task executon
    pub use futures_core::task::*;
}
