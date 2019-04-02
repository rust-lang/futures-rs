//! Asynchronous sinks
//!
//! This crate contains the `Sink` trait which allows values to be sent
//! asynchronously.

#![cfg_attr(not(feature = "std"), no_std)]
#![warn(missing_docs, missing_debug_implementations, rust_2018_idioms)]
#![doc(html_root_url = "https://rust-lang-nursery.github.io/futures-api-docs/0.3.0-alpha.13/futures_sink")]

#![feature(futures_api)]
#![cfg_attr(all(feature = "alloc", not(feature = "std")), feature(alloc))]

#[cfg(all(feature = "alloc", not(any(feature = "std", feature = "nightly"))))]
compile_error!("The `alloc` feature without `std` requires the `nightly` feature active to explicitly opt-in to unstable features");

#[cfg(all(feature = "alloc", not(feature = "std")))]
extern crate alloc;
#[cfg(feature = "std")]
extern crate std as alloc;

use futures_core::task::{Waker, Poll};
use core::pin::Pin;

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
/// the simplest way to ultimately consume a stream.
#[must_use = "sinks do nothing unless polled"]
pub trait Sink<Item> {
    /// The type of value produced by the sink when an error occurs.
    type SinkError;

    /// Attempts to prepare the `Sink` to receive a value.
    ///
    /// This method must be called and return `Ok(Poll::Ready(()))` prior to
    /// each call to `start_send`.
    ///
    /// This method returns `Poll::Ready` once the underlying sink is ready to
    /// receive data. If this method returns `Poll::Pending`, the current task
    /// is registered to be notified (via `waker.wake()`) when `poll_ready`
    /// should be called again.
    ///
    /// In most cases, if the sink encounters an error, the sink will
    /// permanently be unable to receive items.
    fn poll_ready(self: Pin<&mut Self>, waker: &Waker) -> Poll<Result<(), Self::SinkError>>;

    /// Begin the process of sending a value to the sink.
    /// Each call to this function must be proceeded by a successful call to
    /// `poll_ready` which returned `Ok(Poll::Ready(()))`.
    ///
    /// As the name suggests, this method only *begins* the process of sending
    /// the item. If the sink employs buffering, the item isn't fully processed
    /// until the buffer is fully flushed. Since sinks are designed to work with
    /// asynchronous I/O, the process of actually writing out the data to an
    /// underlying object takes place asynchronously. **You *must* use
    /// `poll_flush` or `poll_close` in order to guarantee completion of a
    /// send**.
    ///
    /// Implementations of `poll_ready` and `start_send` will usually involve
    /// flushing behind the scenes in order to make room for new messages.
    /// It is only necessary to call `poll_flush` if you need to guarantee that
    /// *all* of the items placed into the `Sink` have been sent.
    ///
    /// In most cases, if the sink encounters an error, the sink will
    /// permanently be unable to receive items.
    fn start_send(self: Pin<&mut Self>, item: Item)
                  -> Result<(), Self::SinkError>;

    /// Flush any remaining output from this sink.
    ///
    /// Returns `Ok(Poll::Ready(()))` when no buffered items remain. If this
    /// value is returned then it is guaranteed that all previous values sent
    /// via `start_send` have been flushed.
    ///
    /// Returns `Ok(Poll::Pending)` if there is more work left to do, in which
    /// case the current task is scheduled (via `waker.wake()`) to wake up when
    /// `poll_flush` should be called again.
    ///
    /// In most cases, if the sink encounters an error, the sink will
    /// permanently be unable to receive items.
    fn poll_flush(self: Pin<&mut Self>, waker: &Waker) -> Poll<Result<(), Self::SinkError>>;

    /// Flush any remaining output and close this sink, if necessary.
    ///
    /// Returns `Ok(Poll::Ready(()))` when no buffered items remain and the sink
    /// has been successfully closed.
    ///
    /// Returns `Ok(Poll::Pending)` if there is more work left to do, in which
    /// case the current task is scheduled (via `waker.wake()`) to wake up when
    /// `poll_close` should be called again.
    ///
    /// If this function encounters an error, the sink should be considered to
    /// have failed permanently, and no more `Sink` methods should be called.
    fn poll_close(self: Pin<&mut Self>, waker: &Waker) -> Poll<Result<(), Self::SinkError>>;
}

impl<'a, S: ?Sized + Sink<Item> + Unpin, Item> Sink<Item> for &'a mut S {
    type SinkError = S::SinkError;

    fn poll_ready(mut self: Pin<&mut Self>, waker: &Waker) -> Poll<Result<(), Self::SinkError>> {
        Pin::new(&mut **self).poll_ready(waker)
    }

    fn start_send(mut self: Pin<&mut Self>, item: Item) -> Result<(), Self::SinkError> {
        Pin::new(&mut **self).start_send(item)
    }

    fn poll_flush(mut self: Pin<&mut Self>, waker: &Waker) -> Poll<Result<(), Self::SinkError>> {
        Pin::new(&mut **self).poll_flush(waker)
    }

    fn poll_close(mut self: Pin<&mut Self>, waker: &Waker) -> Poll<Result<(), Self::SinkError>> {
        Pin::new(&mut **self).poll_close(waker)
    }
}

impl<'a, S: ?Sized + Sink<Item>, Item> Sink<Item> for Pin<&'a mut S> {
    type SinkError = S::SinkError;

    fn poll_ready(mut self: Pin<&mut Self>, waker: &Waker) -> Poll<Result<(), Self::SinkError>> {
        S::poll_ready((*self).as_mut(), waker)
    }

    fn start_send(mut self: Pin<&mut Self>, item: Item) -> Result<(), Self::SinkError> {
        S::start_send((*self).as_mut(), item)
    }

    fn poll_flush(mut self: Pin<&mut Self>, waker: &Waker) -> Poll<Result<(), Self::SinkError>> {
        S::poll_flush((*self).as_mut(), waker)
    }

    fn poll_close(mut self: Pin<&mut Self>, waker: &Waker) -> Poll<Result<(), Self::SinkError>> {
        S::poll_close((*self).as_mut(), waker)
    }
}

#[cfg(feature = "std")]
mod channel_impls;

#[cfg(feature = "alloc")]
mod if_alloc {
    use super::*;

    /// The error type for `Vec` and `VecDequeue` when used as `Sink`s.
    /// Values of this type can never be created.
    #[derive(Copy, Clone, Debug)]
    pub enum VecSinkError {}

    impl<T> Sink<T> for ::alloc::vec::Vec<T> {
        type SinkError = VecSinkError;

        fn poll_ready(self: Pin<&mut Self>, _: &Waker) -> Poll<Result<(), Self::SinkError>> {
            Poll::Ready(Ok(()))
        }

        fn start_send(self: Pin<&mut Self>, item: T) -> Result<(), Self::SinkError> {
            // TODO: impl<T> Unpin for Vec<T> {}
            unsafe { Pin::get_unchecked_mut(self) }.push(item);
            Ok(())
        }

        fn poll_flush(self: Pin<&mut Self>, _: &Waker) -> Poll<Result<(), Self::SinkError>> {
            Poll::Ready(Ok(()))
        }

        fn poll_close(self: Pin<&mut Self>, _: &Waker) -> Poll<Result<(), Self::SinkError>> {
            Poll::Ready(Ok(()))
        }
    }

    impl<T> Sink<T> for ::alloc::collections::VecDeque<T> {
        type SinkError = VecSinkError;

        fn poll_ready(self: Pin<&mut Self>, _: &Waker) -> Poll<Result<(), Self::SinkError>> {
            Poll::Ready(Ok(()))
        }

        fn start_send(self: Pin<&mut Self>, item: T) -> Result<(), Self::SinkError> {
            // TODO: impl<T> Unpin for Vec<T> {}
            unsafe { Pin::get_unchecked_mut(self) }.push_back(item);
            Ok(())
        }

        fn poll_flush(self: Pin<&mut Self>, _: &Waker) -> Poll<Result<(), Self::SinkError>> {
            Poll::Ready(Ok(()))
        }

        fn poll_close(self: Pin<&mut Self>, _: &Waker) -> Poll<Result<(), Self::SinkError>> {
            Poll::Ready(Ok(()))
        }
    }

    impl<S: ?Sized + Sink<Item> + Unpin, Item> Sink<Item> for ::alloc::boxed::Box<S> {
        type SinkError = S::SinkError;

        fn poll_ready(mut self: Pin<&mut Self>, waker: &Waker) -> Poll<Result<(), Self::SinkError>> {
            Pin::new(&mut **self).poll_ready(waker)
        }

        fn start_send(mut self: Pin<&mut Self>, item: Item) -> Result<(), Self::SinkError> {
            Pin::new(&mut **self).start_send(item)
        }

        fn poll_flush(mut self: Pin<&mut Self>, waker: &Waker) -> Poll<Result<(), Self::SinkError>> {
            Pin::new(&mut **self).poll_flush(waker)
        }

        fn poll_close(mut self: Pin<&mut Self>, waker: &Waker) -> Poll<Result<(), Self::SinkError>> {
            Pin::new(&mut **self).poll_close(waker)
        }
    }
}

#[cfg(feature = "alloc")]
pub use self::if_alloc::*;

#[cfg(feature = "either")]
use either::Either;
#[cfg(feature = "either")]
impl<A, B, Item> Sink<Item> for Either<A, B>
    where A: Sink<Item>,
          B: Sink<Item, SinkError=A::SinkError>,
{
    type SinkError = A::SinkError;

    fn poll_ready(self: Pin<&mut Self>, waker: &Waker) -> Poll<Result<(), Self::SinkError>> {
        unsafe {
            match Pin::get_unchecked_mut(self) {
                Either::Left(x) => Pin::new_unchecked(x).poll_ready(waker),
                Either::Right(x) => Pin::new_unchecked(x).poll_ready(waker),
            }
        }
    }

    fn start_send(self: Pin<&mut Self>, item: Item) -> Result<(), Self::SinkError> {
        unsafe {
            match Pin::get_unchecked_mut(self) {
                Either::Left(x) => Pin::new_unchecked(x).start_send(item),
                Either::Right(x) => Pin::new_unchecked(x).start_send(item),
            }
        }
    }

    fn poll_flush(self: Pin<&mut Self>, waker: &Waker) -> Poll<Result<(), Self::SinkError>> {
        unsafe {
            match Pin::get_unchecked_mut(self) {
                Either::Left(x) => Pin::new_unchecked(x).poll_flush(waker),
                Either::Right(x) => Pin::new_unchecked(x).poll_flush(waker),
            }
        }
    }

    fn poll_close(self: Pin<&mut Self>, waker: &Waker) -> Poll<Result<(), Self::SinkError>> {
        unsafe {
            match Pin::get_unchecked_mut(self) {
                Either::Left(x) => Pin::new_unchecked(x).poll_close(waker),
                Either::Right(x) => Pin::new_unchecked(x).poll_close(waker),
            }
        }
    }
}
