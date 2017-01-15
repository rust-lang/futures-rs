//! Asynchronous sinks
//!
//! This module contains the `Sink` trait, along with a number of adapter types
//! for it. An overview is available in the documentaiton for the trait itself.
//!
//! You can find more information/tutorials about streams [online at
//! https://tokio.rs][online]
//!
//! [online]: https://tokio.rs/docs/getting-started/streams-and-sinks/

use {IntoFuture, Poll, StartSend};
use stream::Stream;

mod with;
// mod with_map;
// mod with_filter;
// mod with_filter_map;
mod flush;
mod send;
mod send_all;

if_std! {
    mod buffer;

    pub use self::buffer::Buffer;

    // TODO: consider expanding this via e.g. FromIterator
    impl<T> Sink for ::std::vec::Vec<T> {
        type SinkItem = T;
        type SinkError = (); // Change this to ! once it stabilizes

        fn start_send(&mut self, item: Self::SinkItem)
                      -> StartSend<Self::SinkItem, Self::SinkError>
        {
            self.push(item);
            Ok(::AsyncSink::Ready)
        }

        fn poll_complete(&mut self) -> Poll<(), Self::SinkError> {
            Ok(::Async::Ready(()))
        }
    }

    /// A type alias for `Box<Stream + Send>`
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
    }
}

pub use self::with::With;
pub use self::flush::Flush;
pub use self::send::Send;
pub use self::send_all::SendAll;

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
    /// The method returns `AsyncSink::NotReady` if the sink was unable to begin
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
    /// - A previous call to `start_send` or `poll_complete` yielded a permanent
    /// error.
    fn start_send(&mut self, item: Self::SinkItem)
                  -> StartSend<Self::SinkItem, Self::SinkError>;

    /// Make progress on all pending requests, and determine whether they have
    /// completed.
    ///
    /// Since sinks are asynchronous, no single method completes all of their
    /// work in one shot. Instead, you use `poll_complete` to repeatedly drive
    /// the sink to make progress on requests (such as `start_send`). As with
    /// `Future::poll`, if the pending requests are not able to complete during
    /// this call, the current task is automatically scheduled to be woken up
    /// again once more progress is possible.
    ///
    /// # Return value
    ///
    /// Returns `Ok(Async::Ready(()))` when no unprocessed requests remain.
    ///
    /// Returns `Ok(Async::NotReady)` if there is more work left to do, in which
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
    /// - A previous call to `start_send` or `poll_complete` yielded a permanent
    /// error.
    fn poll_complete(&mut self) -> Poll<(), Self::SinkError>;

    /// Composes a function *in front of* the sink.
    ///
    /// This adapter produces a new sink that passes each value through the
    /// given function `f` before sending it to `self`.
    ///
    /// To process each value, `f` produces a *future*, which is then polled to
    /// completion before passing its result down to the underlying sink. If the
    /// future produces an error, that error is returned by the new sink.
    ///
    /// Note that this function consumes the given sink, returning a wrapped
    /// version, much like `Iterator::map`.
    fn with<U, F, Fut>(self, f: F) -> With<Self, U, F, Fut>
        where F: FnMut(U) -> Fut,
              Fut: IntoFuture<Item = Self::SinkItem>,
              Fut::Error: From<Self::SinkError>,
              Self: Sized
    {
        with::new(self, f)
    }

    /*
    fn with_map<U, F>(self, f: F) -> WithMap<Self, U, F>
        where F: FnMut(U) -> Self::SinkItem,
              Self: Sized;

    fn with_filter<F>(self, f: F) -> WithFilter<Self, F>
        where F: FnMut(Self::SinkItem) -> bool,
              Self: Sized;

    fn with_filter_map<U, F>(self, f: F) -> WithFilterMap<Self, U, F>
        where F: FnMut(U) -> Option<Self::SinkItem>,
              Self: Sized;
     */

    /// Adds a fixed-size buffer to the current sink.
    ///
    /// The resulting sink will buffer up to `amt` items when the underlying
    /// sink is unwilling to accept additional items. Calling `poll_complete` on
    /// the buffered sink will attempt to both empty the buffer and complete
    /// processing on the underlying sink.
    ///
    /// Note that this function consumes the given sink, returning a wrapped
    /// version, much like `Iterator::map`.
    ///
    /// This method is only available when the `use_std` feature of this
    /// library is activated, and it is activated by default.
    #[cfg(feature = "use_std")]
    fn buffer(self, amt: usize) -> Buffer<Self>
        where Self: Sized
    {
        buffer::new(self, amt)
    }

    /// A future that completes when the sink has finished processing all
    /// pending requests.
    ///
    /// The sink itself is returned after flushing is complete; this adapter is
    /// intended to be used when you want to stop sending to the sink until
    /// all current requests are processed.
    fn flush(self) -> Flush<Self>
        where Self: Sized
    {
        flush::new(self)
    }

    /// A future that completes after the given item has been fully processed
    /// into the sink, including flushing.
    ///
    /// Note that, **because of the flushing requirement, it is usually better
    /// to batch together items to send via `send_all`, rather than flushing
    /// between each item.**
    ///
    /// On completion, the sink is returned.
    fn send(self, item: Self::SinkItem) -> Send<Self>
        where Self: Sized
    {
        send::new(self, item)
    }

    /// A future that completes after the given stream has been fully processed
    /// into the sink, including flushing.
    ///
    /// This future will drive the stream to keep producing items until it is
    /// exhausted, sending each item to the sink. It will complete once both the
    /// stream is exhausted, and the sink has fully processed and flushed all of
    /// the items sent to it.
    ///
    /// Doing `sink.send_all(stream)` is roughly equivalent to
    /// `stream.forward(sink)`.
    ///
    /// On completion, the pair `(sink, source)` is returned.
    fn send_all<S>(self, stream: S) -> SendAll<Self, S>
        where S: Stream<Item = Self::SinkItem>,
              Self::SinkError: From<S::Error>,
              Self: Sized
    {
        send_all::new(self, stream)
    }
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
}
