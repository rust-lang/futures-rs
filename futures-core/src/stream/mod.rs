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

use Poll;

if_std! {
    impl<S: ?Sized + Stream> Stream for ::std::boxed::Box<S> {
        type Item = S::Item;
        type Error = S::Error;

        fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
            (**self).poll()
        }
    }

    impl<S: Stream> Stream for ::std::panic::AssertUnwindSafe<S> {
        type Item = S::Item;
        type Error = S::Error;

        fn poll(&mut self) -> Poll<Option<S::Item>, S::Error> {
            self.0.poll()
        }
    }
}

/// A stream of values, not all of which may have been produced yet.
///
/// `Stream` is a trait to represent any source of sequential events or items
/// which acts like an iterator but long periods of time may pass between
/// items. Like `Future` the methods of `Stream` never block and it is thus
/// suitable for programming in an asynchronous fashion. This trait is very
/// similar to the `Iterator` trait in the standard library where `Some` is
/// used to signal elements of the stream and `None` is used to indicate that
/// the stream is finished.
///
/// Like futures a stream has basic combinators to transform the stream, perform
/// more work on each item, etc.
///
/// You can find more information/tutorials about streams [online at
/// https://tokio.rs][online]
///
/// [online]: https://tokio.rs/docs/getting-started/streams-and-sinks/
///
/// # Streams as Futures
///
/// Any instance of `Stream` can also be viewed as a `Future` where the resolved
/// value is the next item in the stream along with the rest of the stream. The
/// `into_future` adaptor can be used here to convert any stream into a future
/// for use with other future methods like `join` and `select`.
///
/// # Errors
///
/// Streams, like futures, can also model errors in their computation. All
/// streams have an associated `Error` type like with futures. Currently as of
/// the 0.1 release of this library an error on a stream **does not terminate
/// the stream**. That is, after one error is received, another error may be
/// received from the same stream (it's valid to keep polling).
///
/// This property of streams, however, is [being considered] for change in 0.2
/// where an error on a stream is similar to `None`, it terminates the stream
/// entirely. If one of these use cases suits you perfectly and not the other,
/// please feel welcome to comment on [the issue][being considered]!
///
/// [being considered]: https://github.com/alexcrichton/futures-rs/issues/206
pub trait Stream {
    /// The type of item this stream will yield on success.
    type Item;

    /// The type of error this stream may generate.
    type Error;

    /// Attempt to pull out the next value of this stream, returning `None` if
    /// the stream is finished.
    ///
    /// This method, like `Future::poll`, is the sole method of pulling out a
    /// value from a stream. This method must also be run within the context of
    /// a task typically and implementors of this trait must ensure that
    /// implementations of this method do not block, as it may cause consumers
    /// to behave badly.
    ///
    /// # Return value
    ///
    /// If `Pending` is returned then this stream's next value is not ready
    /// yet and implementations will ensure that the current task will be
    /// notified when the next value may be ready. If `Some` is returned then
    /// the returned value represents the next value on the stream. `Err`
    /// indicates an error happened, while `Ok` indicates whether there was a
    /// new item on the stream or whether the stream has terminated.
    ///
    /// # Panics
    ///
    /// Once a stream is finished, that is `Ready(None)` has been returned,
    /// further calls to `poll` may result in a panic or other "bad behavior".
    /// If this is difficult to guard against then the `fuse` adapter can be
    /// used to ensure that `poll` always has well-defined semantics.
    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error>;
}

impl<'a, S: ?Sized + Stream> Stream for &'a mut S {
    type Item = S::Item;
    type Error = S::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        (**self).poll()
    }
}
