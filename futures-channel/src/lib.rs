//! Asynchronous channels
//!
//! This crate provides channels which can be used to communicate between futures.

#![deny(missing_docs, missing_debug_implementations)]
#![doc(html_root_url = "https://docs.rs/futures/0.2")]

#[macro_use]
extern crate futures_core;

if_std! {
    use futures_core::{Future, Stream, Poll, Async};
    use futures_core::task::{self, Task};

    mod lock;
    pub mod mpsc;
    pub mod oneshot;
}

// TODO: these types should be moved to futures-sink when it stabilizes

#[doc(hidden)]
/// The result of an asynchronous attempt to send a value to a sink.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum AsyncSink<T> {
    /// The `start_send` attempt succeeded, so the sending process has
    /// *started*; you must use `Sink::poll_complete` to drive the send
    /// to completion.
    Ready,

    /// The `start_send` attempt failed due to the sink being full. The value
    /// being sent is returned, and the current `Task` will be automatically
    /// notified again once the sink has room.
    Pending(T),
}

impl<T> AsyncSink<T> {
    /// Change the Pending value of this `AsyncSink` with the closure provided
    pub fn map<F, U>(self, f: F) -> AsyncSink<U>
        where F: FnOnce(T) -> U,
    {
        match self {
            AsyncSink::Ready => AsyncSink::Ready,
            AsyncSink::Pending(t) => AsyncSink::Pending(f(t)),
        }
    }

    /// Returns whether this is `AsyncSink::Ready`
    pub fn is_ready(&self) -> bool {
        match *self {
            AsyncSink::Ready => true,
            AsyncSink::Pending(_) => false,
        }
    }

    /// Returns whether this is `AsyncSink::Pending`
    pub fn is_not_ready(&self) -> bool {
        !self.is_ready()
    }
}


#[doc(hidden)]
/// Return type of the `Sink::start_send` method, indicating the outcome of a
/// send attempt. See `AsyncSink` for more details.
pub type StartSend<T, E> = Result<AsyncSink<T>, E>;
