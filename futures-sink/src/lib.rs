//! Asynchronous sinks
//!
//! This crate contains the `Sink` trait which allows values to be sent
//! asynchronously.

#![no_std]
#![deny(missing_docs, missing_debug_implementations)]
#![doc(html_root_url = "https://docs.rs/futures/0.2")]

#[cfg(feature = "std")]
extern crate std;

#[macro_use]
extern crate futures_core;

use futures_core::Poll;

if_std! {
    use futures_core::Async;
    use futures_core::never::Never;

    impl<T> Sink for ::std::vec::Vec<T> {
        type SinkItem = T;
        type SinkError = Never;

        fn start_send(&mut self, item: Self::SinkItem)
                      -> StartSend<Self::SinkItem, Self::SinkError>
        {
            self.push(item);
            Ok(::AsyncSink::Ready)
        }

        fn poll_complete(&mut self) -> Poll<(), Self::SinkError> {
            Ok(::Async::Ready(()))
        }

        fn close(&mut self) -> Poll<(), Self::SinkError> {
            Ok(::Async::Ready(()))
        }
    }

    /// A type alias for `Box<Sink + Send>`
    pub type BoxSink<T, E> = ::std::boxed::Box<Sink<SinkItem = T, SinkError = E> +
                                               ::core::marker::Send>;

    impl<S: ?Sized + Sink> Sink for ::std::boxed::Box<S> {
        type SinkItem = S::SinkItem;
        type SinkError = S::SinkError;

        fn start_send(&mut self, item: Self::SinkItem)
                      -> StartSend<Self::SinkItem, Self::SinkError> {
            (**self).start_send(item)
        }

        fn poll_complete(&mut self) -> Poll<(), Self::SinkError> {
            (**self).poll_complete()
        }

        fn close(&mut self) -> Poll<(), Self::SinkError> {
            (**self).close()
        }
    }
}

/// A `Sink` is a value into which other values can be sent, asynchronously.
///
/// Basic examples of sinks include the sending side of:
///
/// - Channels
/// - Sockets
/// - Pipes
///
/// In addition to such "primitive" sinks, it's typical to layer additional
/// functionality, such as buffering, on top of an existing sink.
///
/// Sending to a sink is "asynchronous" in the sense that the value may not be
/// sent in its entirety immediately. Instead, values are sent in a two-phase
/// way: first by initiating a send, and then by polling for completion. This
/// two-phase setup is analogous to buffered writing in synchronous code, where
/// writes often succeed immediately, but internally are buffered and are
/// *actually* written only upon flushing.
///
/// In addition, the `Sink` may be *full*, in which case it is not even possible
/// to start the sending process.
///
/// As with `Future` and `Stream`, the `Sink` trait is built from a few core
/// required methods, and a host of default methods for working in a
/// higher-level way. The `Sink::send_all` combinator is of particular
/// importance: you can use it to send an entire stream to a sink, which is
/// the simplest way to ultimately consume a sink.
///
/// You can find more information/tutorials about streams [online at
/// https://tokio.rs][online]
///
/// [online]: https://tokio.rs/docs/getting-started/streams-and-sinks/
pub trait Sink {
    /// The type of value that the sink accepts.
    type SinkItem;

    /// The type of value produced by the sink when an error occurs.
    type SinkError;

    /// Begin the process of sending a value to the sink.
    ///
    /// As the name suggests, this method only *begins* the process of sending
    /// the item. If the sink employs buffering, the item isn't fully processed
    /// until the buffer is fully flushed. Since sinks are designed to work with
    /// asynchronous I/O, the process of actually writing out the data to an
    /// underlying object takes place asynchronously. **You *must* use
    /// `poll_complete` in order to drive completion of a send**. In particular,
    /// `start_send` does not begin the flushing process
    ///
    /// # Return value
    ///
    /// This method returns `AsyncSink::Ready` if the sink was able to start
    /// sending `item`. In that case, you *must* ensure that you call
    /// `poll_complete` to process the sent item to completion. Note, however,
    /// that several calls to `start_send` can be made prior to calling
    /// `poll_complete`, which will work on completing all pending items.
    ///
    /// The method returns `AsyncSink::Pending` if the sink was unable to begin
    /// sending, usually due to being full. The sink must have attempted to
    /// complete processing any outstanding requests (equivalent to
    /// `poll_complete`) before yielding this result. The current task will be
    /// automatically scheduled for notification when the sink may be ready to
    /// receive new values.
    ///
    /// # Errors
    ///
    /// If the sink encounters an error other than being temporarily full, it
    /// uses the `Err` variant to signal that error. In most cases, such errors
    /// mean that the sink will permanently be unable to receive items.
    ///
    /// # Panics
    ///
    /// This method may panic in a few situations, depending on the specific
    /// sink:
    ///
    /// - It is called outside of the context of a task.
    /// - A previous call to `start_send` or `poll_complete` yielded an error.
    fn start_send(&mut self, item: Self::SinkItem)
                  -> StartSend<Self::SinkItem, Self::SinkError>;

    /// Flush all output from this sink, if necessary.
    ///
    /// Some sinks may buffer intermediate data as an optimization to improve
    /// throughput. In other words, if a sink has a corresponding receiver then
    /// a successful `start_send` above may not guarantee that the value is
    /// actually ready to be received by the receiver. This function is intended
    /// to be used to ensure that values do indeed make their way to the
    /// receiver.
    ///
    /// This function will attempt to process any pending requests on behalf of
    /// the sink and drive it to completion.
    ///
    /// # Return value
    ///
    /// Returns `Ok(Async::Ready(()))` when no buffered items remain. If this
    /// value is returned then it is guaranteed that all previous values sent
    /// via `start_send` will be guaranteed to be available to a listening
    /// receiver.
    ///
    /// Returns `Ok(Async::Pending)` if there is more work left to do, in which
    /// case the current task is scheduled to wake up when more progress may be
    /// possible.
    ///
    /// # Errors
    ///
    /// Returns `Err` if the sink encounters an error while processing one of
    /// its pending requests. Due to the buffered nature of requests, it is not
    /// generally possible to correlate the error with a particular request. As
    /// with `start_send`, these errors are generally "fatal" for continued use
    /// of the sink.
    ///
    /// # Panics
    ///
    /// This method may panic in a few situations, depending on the specific sink:
    ///
    /// - It is called outside of the context of a task.
    /// - A previous call to `start_send` or `poll_complete` yielded an error.
    ///
    /// # Compatibility nodes
    ///
    /// The name of this method may be slightly misleading as the original
    /// intention was to have this method be more general than just flushing
    /// requests. Over time though it was decided to trim back the ambitions of
    /// this method to what it's always done, just flushing.
    ///
    /// In the 0.2 release series of futures this method will be renamed to
    /// `poll_flush`. For 0.1, however, the breaking change is not happening
    /// yet.
    fn poll_complete(&mut self) -> Poll<(), Self::SinkError>;

    /// A method to indicate that no more values will ever be pushed into this
    /// sink.
    ///
    /// This method is used to indicate that a sink will no longer even be given
    /// another value by the caller. That is, the `start_send` method above will
    /// be called no longer (nor `poll_complete`). This method is intended to
    /// model "graceful shutdown" in various protocols where the intent to shut
    /// down is followed by a little more blocking work.
    ///
    /// Callers of this function should work it it in a similar fashion to
    /// `poll_complete`. Once called it may return `Pending` which indicates
    /// that more external work needs to happen to make progress. The current
    /// task will be scheduled to receive a notification in such an event,
    /// however.
    ///
    /// Note that this function will imply `poll_complete` above. That is, if a
    /// sink has buffered data, then it'll be flushed out during a `close`
    /// operation. It is not necessary to have `poll_complete` return `Ready`
    /// before a `close` is called. Once a `close` is called, though,
    /// `poll_complete` cannot be called.
    ///
    /// # Return value
    ///
    /// This function, like `poll_complete`, returns a `Poll`. The value is
    /// `Ready` once the close operation has completed. At that point it should
    /// be safe to drop the sink and deallocate associated resources.
    ///
    /// If the value returned is `Pending` then the sink is not yet closed and
    /// work needs to be done to close it. The work has been scheduled and the
    /// current task will receive a notification when it's next ready to call
    /// this method again.
    ///
    /// Finally, this function may also return an error.
    ///
    /// # Errors
    ///
    /// This function will return an `Err` if any operation along the way during
    /// the close operation fails. An error typically is fatal for a sink and is
    /// unable to be recovered from, but in specific situations this may not
    /// always be true.
    ///
    /// Note that it's also typically an error to call `start_send` or
    /// `poll_complete` after the `close` function is called. This method will
    /// *initiate* a close, and continuing to send values after that (or attempt
    /// to flush) may result in strange behavior, panics, errors, etc. Once this
    /// method is called, it must be the only method called on this `Sink`.
    ///
    /// # Panics
    ///
    /// This method may panic or cause panics if:
    ///
    /// * It is called outside the context of a future's task
    /// * It is called and then `start_send` or `poll_complete` is called
    ///
    /// # Compatibility notes
    ///
    /// Note that this function is currently by default a provided function,
    /// defaulted to calling `poll_complete` above. This function was added
    /// in the 0.1 series of the crate as a backwards-compatible addition. It
    /// is intended that in the 0.2 series the method will no longer be a
    /// default method.
    ///
    /// It is highly recommended to consider this method a required method and
    /// to implement it whenever you implement `Sink` locally. It is especially
    /// crucial to be sure to close inner sinks, if applicable.
    fn close(&mut self) -> Poll<(), Self::SinkError>;
}

impl<'a, S: ?Sized + Sink> Sink for &'a mut S {
    type SinkItem = S::SinkItem;
    type SinkError = S::SinkError;

    fn start_send(&mut self, item: Self::SinkItem)
                  -> StartSend<Self::SinkItem, Self::SinkError> {
        (**self).start_send(item)
    }

    fn poll_complete(&mut self) -> Poll<(), Self::SinkError> {
        (**self).poll_complete()
    }

    fn close(&mut self) -> Poll<(), Self::SinkError> {
        (**self).close()
    }
}

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


/// Return type of the `Sink::start_send` method, indicating the outcome of a
/// send attempt. See `AsyncSink` for more details.
pub type StartSend<T, E> = Result<AsyncSink<T>, E>;
