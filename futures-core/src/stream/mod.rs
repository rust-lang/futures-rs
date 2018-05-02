//! Asynchronous streams.

#[cfg(feature = "nightly")]
use core::mem::Pin;

use {Poll, task, Unpin};

/// A stream of values produced asynchronously.
///
/// If `Future<Output = T>` is an asynchronous version of `T`, then `Stream<Item
/// = T>` is an asynchronous version of `Iterator<Item = T>`. A stream
/// represents a sequence of value-producing events that occur asynchronously to
/// the caller.
///
/// The trait is modeled after `Future`, but allows `poll_next` to be called
/// even after a value has been produced, yielding `None` once the stream has
/// been fully exhausted.
pub trait Stream {
    /// Values yielded by the stream.
    type Item;

    /// Attempt to pull out the next value of this stream, registering the
    /// current task for wakeup if the value is not yet available, and returning
    /// `None` if the stream is exhausted.
    ///
    /// # Return value
    ///
    /// There are several possible return values, each indicating a distinct
    /// stream state:
    ///
    /// - [`Pending`](::Poll) means that this stream's next value is not ready
    /// yet. Implementations will ensure that the current task will be notified
    /// when the next value may be ready.
    ///
    /// - [`Ready(Some(val))`](::Poll) means that the stream has successfully
    /// produced a value, `val`, and may produce further values on subsequent
    /// `poll_next` calls.
    ///
    /// - [`Ready(None)`](::Poll) means that the stream has terminated, and
    /// `poll_next` should not be invoked again.
    ///
    /// # Panics
    ///
    /// Once a stream is finished, i.e. `Ready(None)` has been returned, further
    /// calls to `poll_next` may result in a panic or other "bad behavior".  If this
    /// is difficult to guard against then the `fuse` adapter can be used to
    /// ensure that `poll_next` always returns `Ready(None)` in subsequent calls.
    #[cfg(feature = "nightly")]
    fn poll_next(self: Pin<Self>, cx: &mut task::Context) -> Poll<Option<Self::Item>>;

    /// A convenience for `poll_next` when a stream is `Unpin`.
    #[cfg(feature = "nightly")]
    fn poll_next_mut(&mut self, cx: &mut task::Context) -> Poll<Option<Self::Item>>
        where Self: Unpin
    {
        Pin::new(self).poll_next(cx)
    }

    /// Attempt to pull out the next value of this stream, registering the
    /// current task for wakeup if the value is not yet available, and returning
    /// `None` if the stream is exhausted.
    ///
    /// # Return value
    ///
    /// There are several possible return values, each indicating a distinct
    /// stream state:
    ///
    /// - [`Pending`](::Poll) means that this stream's next value is not ready
    /// yet. Implementations will ensure that the current task will be notified
    /// when the next value may be ready.
    ///
    /// - [`Ready(Some(val))`](::Poll) means that the stream has successfully
    /// produced a value, `val`, and may produce further values on subsequent
    /// `poll_next_mut` calls.
    ///
    /// - [`Ready(None)`](::Poll) means that the stream has terminated, and
    /// `poll_next_mut` should not be invoked again.
    ///
    /// # Panics
    ///
    /// Once a stream is finished, i.e. `Ready(None)` has been returned, further
    /// calls to `poll_next_mut` may result in a panic or other "bad behavior".
    /// If this is difficult to guard against then the `fuse` adapter can be
    /// used to ensure that `poll_next_mut` always returns `Ready(None)` in
    /// subsequent calls.
    #[cfg(not(feature = "nightly"))]
    fn poll_next_mut(&mut self, cx: &mut task::Context) -> Poll<Option<Self::Item>>
        where Self: Unpin;

    #[cfg(not(feature = "nightly"))]
    #[doc(hidden)]
    fn __must_impl_via_unpinned_macro();
}
