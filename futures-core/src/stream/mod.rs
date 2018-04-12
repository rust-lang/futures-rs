//! Asynchronous streams.

use Poll;
use task;

/// A stream of values produced asynchronously.
///
/// If `Future` is an asynchronous version of `Result`, then `Stream` is an
/// asynchronous version of `Iterator`. A stream represents a sequence of
/// value-producing events that occur asynchronously to the caller.
///
/// The trait is modeled after `Future`, but allows `poll_next` to be called
/// even after a value has been produced, yielding `None` once the stream has
/// been fully exhausted.
///
/// # Errors
///
/// Streams, like futures, also bake in errors through an associated `Error`
/// type. An error on a stream **does not terminate the stream**. That is,
/// after one error is received, another value may be received from the same
/// stream (it's valid to keep polling). Thus a stream is somewhat like an
/// `Iterator<Item = Result<T, E>>`, and is always terminated by returning
/// `None`.
pub trait Stream {
    /// Values yielded by the stream.
    type Item;

    /// Errors yielded by the stream.
    type Error;

    /// Attempt to pull out the next value of this stream, registering the
    /// current task for wakeup if the value is not yet available, and returning
    /// `None` if the stream is exhausted.
    ///
    /// # Return value
    ///
    /// There are several possible return values, each indicating a distinct
    /// stream state:
    ///
    /// - [`Ok(Pending)`](::Async) means that this stream's next value is not
    /// ready yet. Implementations will ensure that the current task will be
    /// notified when the next value may be ready.
    ///
    /// - [`Ok(Ready(Some(val)))`](::Async) means that the stream has
    /// successfully produced a value, `val`, and may produce further values
    /// on subsequent `poll_next` calls.
    ///
    /// - [`Ok(Ready(None))`](::Async) means that the stream has terminated, and
    /// `poll_next` should not be invoked again.
    ///
    /// - `Err(err)` means that the stream encountered an error while trying to
    /// `poll_next`. Subsequent calls to `poll_next` *are* allowed, and may
    /// return further values or errors.
    ///
    /// # Panics
    ///
    /// Once a stream is finished, i.e. `Ready(None)` has been returned, further
    /// calls to `poll_next` may result in a panic or other "bad behavior".  If this
    /// is difficult to guard against then the `fuse` adapter can be used to
    /// ensure that `poll_next` always returns `Ready(None)` in subsequent calls.
    fn poll_next(&mut self, cx: &mut task::Context) -> Poll<Option<Self::Item>, Self::Error>;
}

impl<'a, S: ?Sized + Stream> Stream for &'a mut S {
    type Item = S::Item;
    type Error = S::Error;

    fn poll_next(&mut self, cx: &mut task::Context) -> Poll<Option<Self::Item>, Self::Error> {
        (**self).poll_next(cx)
    }
}

if_std! {
    use Async;
    use never::Never;

    impl<S: ?Sized + Stream> Stream for ::std::boxed::Box<S> {
        type Item = S::Item;
        type Error = S::Error;

        fn poll_next(&mut self, cx: &mut task::Context) -> Poll<Option<Self::Item>, Self::Error> {
            (**self).poll_next(cx)
        }
    }

    #[cfg(feature = "nightly")]
    impl<S: ?Sized + Stream> Stream for ::std::boxed::PinBox<S> {
        type Item = S::Item;
        type Error = S::Error;

        fn poll_next(&mut self, cx: &mut task::Context) -> Poll<Option<Self::Item>, Self::Error> {
            unsafe { ::core::mem::Pin::get_mut(&mut self.as_pin()).poll_next(cx) }
        }
    }

    impl<S: Stream> Stream for ::std::panic::AssertUnwindSafe<S> {
        type Item = S::Item;
        type Error = S::Error;

        fn poll_next(&mut self, cx: &mut task::Context) -> Poll<Option<S::Item>, S::Error> {
            self.0.poll_next(cx)
        }
    }

    impl<T> Stream for ::std::collections::VecDeque<T> {
        type Item = T;
        type Error = Never;

        fn poll_next(&mut self, _cx: &mut task::Context) -> Poll<Option<Self::Item>, Self::Error> {
            Ok(Async::Ready(self.pop_front()))
        }
    }
}
