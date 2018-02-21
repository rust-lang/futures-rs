//! Asynchronous sinks
//!
//! This crate contains the `Sink` trait which allows values to be sent
//! asynchronously.

#![no_std]
#![deny(missing_docs, missing_debug_implementations)]
#![doc(html_root_url = "https://docs.rs/futures-sink/0.2")]

#[cfg(feature = "std")]
extern crate std;

extern crate futures_core;
#[cfg(feature = "std")]
extern crate futures_channel;

macro_rules! if_std {
    ($($i:item)*) => ($(
        #[cfg(feature = "std")]
        $i
    )*)
}

use futures_core::{Async, Poll, task};

if_std! {
    mod channel_impls;

    use futures_core::never::Never;

    impl<T> Sink for ::std::vec::Vec<T> {
        type SinkItem = T;
        type SinkError = Never;

        fn poll_ready(&mut self, _: &mut task::Context) -> Poll<(), Self::SinkError> {
            Ok(Async::Ready(()))
        }

        fn start_send(&mut self, item: Self::SinkItem) -> Result<(), Self::SinkError> {
            self.push(item);
            Ok(())
        }

        fn poll_flush(&mut self, _: &mut task::Context) -> Poll<(), Self::SinkError> {
            Ok(Async::Ready(()))
        }

        fn poll_close(&mut self, _: &mut task::Context) -> Poll<(), Self::SinkError> {
            Ok(Async::Ready(()))
        }
    }

    /// A type alias for `Box<Sink + Send>`
    pub type BoxSink<T, E> = ::std::boxed::Box<Sink<SinkItem = T, SinkError = E> +
                                               ::core::marker::Send>;

    impl<S: ?Sized + Sink> Sink for ::std::boxed::Box<S> {
        type SinkItem = S::SinkItem;
        type SinkError = S::SinkError;

        fn poll_ready(&mut self, cx: &mut task::Context) -> Poll<(), Self::SinkError> {
            (**self).poll_ready(cx)
        }

        fn start_send(&mut self, item: Self::SinkItem) -> Result<(), Self::SinkError> {
            (**self).start_send(item)
        }

        fn poll_flush(&mut self, cx: &mut task::Context) -> Poll<(), Self::SinkError> {
            (**self).poll_flush(cx)
        }

        fn poll_close(&mut self, cx: &mut task::Context) -> Poll<(), Self::SinkError> {
            (**self).poll_close(cx)
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

    /// Check if the sink is ready to start sending a value.
    fn poll_ready(&mut self, cx: &mut task::Context) -> Poll<(), Self::SinkError>;

    /// Begin the process of sending a value to the sink.
    ///
    /// As the name suggests, this method only *begins* the process of sending
    /// the item. If the sink employs buffering, the item isn't fully processed
    /// until the buffer is fully flushed. Since sinks are designed to work with
    /// asynchronous I/O, the process of actually writing out the data to an
    /// underlying object takes place asynchronously. **You *must* use
    /// `flush` in order to drive completion of a send**. In particular,
    /// `start_send` does not begin the flushing process
    ///
    /// # Return value
    ///
    /// This method returns `AsyncSink::Ready` if the sink was able to start
    /// sending `item`. In that case, you *must* ensure that you call
    /// `flush` to process the sent item to completion. Note, however,
    /// that several calls to `start_send` can be made prior to calling
    /// `flush`, which will work on completing all pending items.
    ///
    /// The method returns `AsyncSink::Pending` if the sink was unable to begin
    /// sending, usually due to being full. The sink must have attempted to
    /// complete processing any outstanding requests (equivalent to
    /// `flush`) before yielding this result. The current task will be
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
    /// - A previous call to `start_send` or `flush` yielded an error.
    fn start_send(&mut self, item: Self::SinkItem)
                  -> Result<(), Self::SinkError>;

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
    /// - A previous call to `start_send` or `flush` yielded an error.
    ///
    /// # Compatibility nodes
    ///
    /// The name of this method may be slightly misleading as the original
    /// intention was to have this method be more general than just flushing
    /// requests. Over time though it was decided to trim back the ambitions of
    /// this method to what it's always done, just flushing.
    ///
    /// In the 0.2 release series of futures this method will be renamed to
    /// `flush`. For 0.1, however, the breaking change is not happening
    /// yet.
    fn poll_flush(&mut self, cx: &mut task::Context) -> Poll<(), Self::SinkError>;

    /// TODO: dox
    fn poll_close(&mut self, cx: &mut task::Context) -> Poll<(), Self::SinkError>;
}

impl<'a, S: ?Sized + Sink> Sink for &'a mut S {
    type SinkItem = S::SinkItem;
    type SinkError = S::SinkError;

    fn poll_ready(&mut self, cx: &mut task::Context) -> Poll<(), Self::SinkError> {
        (**self).poll_ready(cx)
    }

    fn start_send(&mut self, item: Self::SinkItem) -> Result<(), Self::SinkError> {
        (**self).start_send(item)
    }

    fn poll_flush(&mut self, cx: &mut task::Context) -> Poll<(), Self::SinkError> {
        (**self).poll_flush(cx)
    }

    fn poll_close(&mut self, cx: &mut task::Context) -> Poll<(), Self::SinkError> {
        (**self).poll_close(cx)
    }
}
