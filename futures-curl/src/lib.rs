//! A `Future` interface on top of libcurl
//!
//! This crate provides a futures-based interface to the libcurl HTTP library.
//! Building on top of the `curl` crate on crates.io, this allows using a
//! battle-tested C library for sending HTTP requests in an asynchronous
//! fashion.
//!
//! # Examples
//!
//! ```rust
//! extern crate curl;
//! extern crate futures;
//! extern crate futures_curl;
//! extern crate futures_mio;
//!
//! use std::io::{self, Write};
//!
//! use curl::easy::Easy;
//! use futures::Future;
//! use futures_mio::Loop;
//! use futures_curl::Session;
//!
//! fn main() {
//!     // Create an event loop that we'll run on, as well as an HTTP `Session`
//!     // which we'll be routing all requests through.
//!     let mut lp = Loop::new().unwrap();
//!     let session = Session::new(lp.pin());
//!
//!     // Prepare the HTTP request to be sent.
//!     let mut req = Easy::new();
//!     req.get(true).unwrap();
//!     req.url("https://www.rust-lang.org").unwrap();
//!     req.write_function(|data| {
//!         io::stdout().write_all(data).unwrap();
//!         Ok(data.len())
//!     }).unwrap();
//!
//!     // Once we've got our session, issue an HTTP request to download the
//!     // rust-lang home page
//!     let request = session.perform(req);
//!
//!     // Execute the request, and print the response code as well as the error
//!     // that happened (if any).
//!     let (mut req, err) = lp.run(request).unwrap();
//!     println!("{:?} {:?}", req.response_code(), err);
//! }
//! ```
//!
//! # Platform support
//!
//! This crate works on both Unix and Windows, but note that it will not scale
//! well on Windows. Unfortunately the implementation (seemingly from libcurl)
//! relies on `select`, which does not scale very far on Windows.

#![deny(missing_docs)]

// TODO: handle level a bit better by turning the event loop every so often

#[macro_use]
extern crate log;
#[macro_use]
extern crate futures;
extern crate futures_mio;
extern crate curl;

#[macro_use]
#[cfg(unix)]
extern crate scoped_tls;
#[macro_use]
#[cfg(unix)]
extern crate futures_io;

#[cfg(windows)]
#[path = "windows.rs"]
mod imp;
#[cfg(unix)]
#[path = "unix.rs"]
mod imp;

use std::io;

use futures::{Future, Poll};
use curl::Error;
use curl::easy::Easy;
use futures_mio::LoopPin;

/// A shared cache for HTTP requests to pool data such as TCP connections
/// between.
///
/// All HTTP requests in this crate are performed through a `Session` type. A
/// `Session` can be cloned to acquire multiple references to the same session.
///
/// Sessions are created through the `Session::new` method, which returns a
/// future that will resolve to a session once it's been initialized.
#[derive(Clone)]
pub struct Session {
    inner: imp::Session,
}

/// A future returned from the `Session::perform` method.
///
/// This future represents the execution of an entire HTTP request. This future
/// will resolve to the original `Easy` handle provided once the HTTP request is
/// complete so metadata about the request can be inspected.
pub struct Perform {
    inner: imp::Perform,
}

impl Session {
    /// Creates a new HTTP session object which will be bound to the given event
    /// loop.
    ///
    /// When using libcurl it will provide us with file descriptors to listen
    /// for events on, so we'll need raw access to an actual event loop in order
    /// to hook up all the pieces together. The event loop will also be the I/O
    /// home for this HTTP session. All HTTP I/O will occur on the event loop
    /// thread.
    ///
    /// This function returns a future which will resolve to a `Session` once
    /// it's been initialized.
    pub fn new(pin: LoopPin) -> Session {
        Session { inner: imp::Session::new(pin) }
    }

    /// Execute and HTTP request asynchronously, returning a future representing
    /// the request's completion.
    ///
    /// This method will consume the provided `Easy` handle, which should be
    /// configured appropriately to send off an HTTP request. The returned
    /// future will resolve back to the handle once the request is performed,
    /// along with any error that happened along the way.
    ///
    /// The `Item` of the returned future is `(Easy, Option<Error>)` so you can
    /// always get the `Easy` handle back, and the `Error` part of the future is
    /// `io::Error` which represents errors communicating with the event loop or
    /// otherwise fatal errors for the `Easy` provided.
    ///
    /// Note that if the `Perform` future is dropped it will cancel the
    /// outstanding HTTP request, cleaning up all resources associated with it.
    ///
    /// Note that all callbacks associated with the `Easy` handle, for example
    /// the read and write functions, will get executed on the event loop. As a
    /// result you may want to close over `LoopData` in them if you'd like to
    /// collect the results.
    pub fn perform(&self, handle: Easy) -> Perform {
        Perform { inner: self.inner.perform(handle) }
    }
}

impl Future for Perform {
    type Item = (Easy, Option<Error>);
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Self::Item, io::Error> {
        self.inner.poll()
    }
}
