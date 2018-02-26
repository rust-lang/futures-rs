//! Sinks
//!
//! This module contains a number of functions for working with `Sink`s,
//! including the `SinkExt` trait which adds methods to `Sink` types.

use futures_core::{Stream, IntoFuture};
use futures_sink::Sink;

mod close;
mod fanout;
mod flush;
mod err_into;
mod map_err;
mod send;
mod send_all;
mod with;
mod with_flat_map;

if_std! {
    mod buffer;
    pub use self::buffer::Buffer;
}

pub use self::close::{Close, close};
pub use self::fanout::Fanout;
pub use self::flush::{Flush, flush};
pub use self::err_into::SinkErrInto;
pub use self::map_err::SinkMapErr;
pub use self::send::Send;
pub use self::send_all::SendAll;
pub use self::with::With;
pub use self::with_flat_map::WithFlatMap;

impl<T: ?Sized> SinkExt for T where T: Sink {}

/// An extension trait for `Sink`s that provides a variety of convenient
/// combinator functions.
pub trait SinkExt: Sink {
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
    fn with<U, Fut, F>(self, f: F) -> With<Self, U, Fut, F>
        where F: FnMut(U) -> Fut,
              Fut: IntoFuture<Item = Self::SinkItem>,
              Fut::Error: From<Self::SinkError>,
              Self: Sized
    {
        with::new(self, f)
    }

    /// Composes a function *in front of* the sink.
    ///
    /// This adapter produces a new sink that passes each value through the
    /// given function `f` before sending it to `self`.
    ///
    /// To process each value, `f` produces a *stream*, of which each value
    /// is passed to the underlying sink. A new value will not be accepted until
    /// the stream has been drained
    ///
    /// Note that this function consumes the given sink, returning a wrapped
    /// version, much like `Iterator::flat_map`.
    ///
    /// # Examples
    /// ---
    /// Using this function with an iterator through use of the `stream::iter_ok()`
    /// function
    ///
    /// ```
    /// # extern crate futures;
    /// # extern crate futures_channel;
    /// # extern crate futures_executor;
    /// use futures::prelude::*;
    /// use futures::stream;
    /// use futures_channel::mpsc;
    /// use futures_executor::block_on;
    ///
    /// # fn main() {
    /// let (tx, rx) = mpsc::channel::<i32>(5);
    ///
    /// let tx = tx.with_flat_map(|x| {
    ///     stream::iter_ok(vec![42; x].into_iter().map(|y| y))
    /// });
    ///
    /// block_on(tx.send(5)).unwrap();
    /// assert_eq!(block_on(rx.collect()), Ok(vec![42, 42, 42, 42, 42]));
    /// # }
    /// ```
    fn with_flat_map<U, St, F>(self, f: F) -> WithFlatMap<Self, U, St, F>
        where F: FnMut(U) -> St,
              St: Stream<Item = Self::SinkItem, Error=Self::SinkError>,
              Self: Sized
        {
            with_flat_map::new(self, f)
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

    /// Transforms the error returned by the sink.
    fn sink_map_err<E, F>(self, f: F) -> SinkMapErr<Self, F>
        where F: FnOnce(Self::SinkError) -> E,
              Self: Sized,
    {
        map_err::new(self, f)
    }

    /// Map this sink's error to a different error type using the `Into` trait.
    ///
    /// If wanting to map errors of a `Sink + Stream`, use `.sink_err_into().err_into()`.
    fn sink_err_into<E>(self) -> err_into::SinkErrInto<Self, E>
        where Self: Sized,
              Self::SinkError: Into<E>,
    {
        err_into::new(self)
    }


    /// Adds a fixed-size buffer to the current sink.
    ///
    /// The resulting sink will buffer up to `amt` items when the underlying
    /// sink is unwilling to accept additional items. Calling `flush` on
    /// the buffered sink will attempt to both empty the buffer and complete
    /// processing on the underlying sink.
    ///
    /// Note that this function consumes the given sink, returning a wrapped
    /// version, much like `Iterator::map`.
    ///
    /// This method is only available when the `std` feature of this
    /// library is activated, and it is activated by default.
    #[cfg(feature = "std")]
    fn buffer(self, amt: usize) -> Buffer<Self>
        where Self: Sized
    {
        buffer::new(self, amt)
    }

    /// Fanout items to multiple sinks.
    ///
    /// This adapter clones each incoming item and forwards it to both this as well as
    /// the other sink at the same time.
    fn fanout<S>(self, other: S) -> Fanout<Self, S>
        where Self: Sized,
              Self::SinkItem: Clone,
              S: Sink<SinkItem=Self::SinkItem, SinkError=Self::SinkError>
    {
        fanout::new(self, other)
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
    /// stream is exhausted, the sink has received all items, and the sink has
    /// been flushed. Note that the sink is **not** closed.
    ///
    /// Doing `sink.send_all(stream)` is roughly equivalent to
    /// `stream.forward(sink)`. The returned future will exhaust all items from
    /// `stream` and send them to `self`.
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
