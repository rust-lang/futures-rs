//! Asynchronous sinks
//!
//! This crate contains the `Sink` trait which allows values to be sent
//! asynchronously.

#![cfg_attr(not(feature = "std"), no_std)]
#![warn(missing_docs, missing_debug_implementations)]
#![doc(html_root_url = "https://rust-lang-nursery.github.io/futures-api-docs/0.3.0-alpha.12/futures_sink")]

#![feature(futures_api)]

use futures_core::task::{LocalWaker, Poll};
use core::marker::Unpin;
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
pub trait Sink {
    /// The type of value that the sink accepts.
    type SinkItem;

    /// The type of value produced by the sink when an error occurs.
    type SinkError;

    /// Attempts to prepare the `Sink` to receive a value.
    ///
    /// This method must be called and return `Ok(Poll::Ready(()))` prior to
    /// each call to `start_send`.
    ///
    /// This method returns `Poll::Ready` once the underlying sink is ready to
    /// receive data. If this method returns `Async::Pending`, the current task
    /// is registered to be notified (via `lw.waker()`) when `poll_ready`
    /// should be called again.
    ///
    /// In most cases, if the sink encounters an error, the sink will
    /// permanently be unable to receive items.
    fn poll_ready(self: Pin<&mut Self>, lw: &LocalWaker) -> Poll<Result<(), Self::SinkError>>;

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
    fn start_send(self: Pin<&mut Self>, item: Self::SinkItem)
                  -> Result<(), Self::SinkError>;

    /// Flush any remaining output from this sink.
    ///
    /// Returns `Ok(Poll::Ready(()))` when no buffered items remain. If this
    /// value is returned then it is guaranteed that all previous values sent
    /// via `start_send` have been flushed.
    ///
    /// Returns `Ok(Async::Pending)` if there is more work left to do, in which
    /// case the current task is scheduled (via `lw.waker()`) to wake up when
    /// `poll_flush` should be called again.
    ///
    /// In most cases, if the sink encounters an error, the sink will
    /// permanently be unable to receive items.
    fn poll_flush(self: Pin<&mut Self>, lw: &LocalWaker) -> Poll<Result<(), Self::SinkError>>;

    /// Flush any remaining output and close this sink, if necessary.
    ///
    /// Returns `Ok(Poll::Ready(()))` when no buffered items remain and the sink
    /// has been successfully closed.
    ///
    /// Returns `Ok(Async::Pending)` if there is more work left to do, in which
    /// case the current task is scheduled (via `lw.waker()`) to wake up when
    /// `poll_close` should be called again.
    ///
    /// If this function encounters an error, the sink should be considered to
    /// have failed permanently, and no more `Sink` methods should be called.
    fn poll_close(self: Pin<&mut Self>, lw: &LocalWaker) -> Poll<Result<(), Self::SinkError>>;
}

impl<'a, S: ?Sized + Sink + Unpin> Sink for &'a mut S {
    type SinkItem = S::SinkItem;
    type SinkError = S::SinkError;

    fn poll_ready(mut self: Pin<&mut Self>, lw: &LocalWaker) -> Poll<Result<(), Self::SinkError>> {
        Pin::new(&mut **self).poll_ready(lw)
    }

    fn start_send(mut self: Pin<&mut Self>, item: Self::SinkItem) -> Result<(), Self::SinkError> {
        Pin::new(&mut **self).start_send(item)
    }

    fn poll_flush(mut self: Pin<&mut Self>, lw: &LocalWaker) -> Poll<Result<(), Self::SinkError>> {
        Pin::new(&mut **self).poll_flush(lw)
    }

    fn poll_close(mut self: Pin<&mut Self>, lw: &LocalWaker) -> Poll<Result<(), Self::SinkError>> {
        Pin::new(&mut **self).poll_close(lw)
    }
}

impl<'a, S: ?Sized + Sink> Sink for Pin<&'a mut S> {
    type SinkItem = S::SinkItem;
    type SinkError = S::SinkError;

    fn poll_ready(mut self: Pin<&mut Self>, lw: &LocalWaker) -> Poll<Result<(), Self::SinkError>> {
        S::poll_ready((*self).as_mut(), lw)
    }

    fn start_send(mut self: Pin<&mut Self>, item: Self::SinkItem) -> Result<(), Self::SinkError> {
        S::start_send((*self).as_mut(), item)
    }

    fn poll_flush(mut self: Pin<&mut Self>, lw: &LocalWaker) -> Poll<Result<(), Self::SinkError>> {
        S::poll_flush((*self).as_mut(), lw)
    }

    fn poll_close(mut self: Pin<&mut Self>, lw: &LocalWaker) -> Poll<Result<(), Self::SinkError>> {
        S::poll_close((*self).as_mut(), lw)
    }
}

#[cfg(feature = "std")]
mod channel_impls;

#[cfg(feature = "std")]
mod if_std {
    use super::*;

    /// The error type for `Vec` and `VecDequeue` when used as `Sink`s.
    /// Values of this type can never be created.
    #[derive(Copy, Clone, Debug)]
    pub enum VecSinkError {}

    impl<T> Sink for ::std::vec::Vec<T> {
        type SinkItem = T;
        type SinkError = VecSinkError;

        fn poll_ready(self: Pin<&mut Self>, _: &LocalWaker) -> Poll<Result<(), Self::SinkError>> {
            Poll::Ready(Ok(()))
        }

        fn start_send(self: Pin<&mut Self>, item: Self::SinkItem) -> Result<(), Self::SinkError> {
            // TODO: impl<T> Unpin for Vec<T> {}
            unsafe { Pin::get_unchecked_mut(self) }.push(item);
            Ok(())
        }

        fn poll_flush(self: Pin<&mut Self>, _: &LocalWaker) -> Poll<Result<(), Self::SinkError>> {
            Poll::Ready(Ok(()))
        }

        fn poll_close(self: Pin<&mut Self>, _: &LocalWaker) -> Poll<Result<(), Self::SinkError>> {
            Poll::Ready(Ok(()))
        }
    }

    impl<T> Sink for ::std::collections::VecDeque<T> {
        type SinkItem = T;
        type SinkError = VecSinkError;

        fn poll_ready(self: Pin<&mut Self>, _: &LocalWaker) -> Poll<Result<(), Self::SinkError>> {
            Poll::Ready(Ok(()))
        }

        fn start_send(self: Pin<&mut Self>, item: Self::SinkItem) -> Result<(), Self::SinkError> {
            // TODO: impl<T> Unpin for Vec<T> {}
            unsafe { Pin::get_unchecked_mut(self) }.push_back(item);
            Ok(())
        }

        fn poll_flush(self: Pin<&mut Self>, _: &LocalWaker) -> Poll<Result<(), Self::SinkError>> {
            Poll::Ready(Ok(()))
        }

        fn poll_close(self: Pin<&mut Self>, _: &LocalWaker) -> Poll<Result<(), Self::SinkError>> {
            Poll::Ready(Ok(()))
        }
    }

    impl<S: ?Sized + Sink + Unpin> Sink for ::std::boxed::Box<S> {
        type SinkItem = S::SinkItem;
        type SinkError = S::SinkError;

        fn poll_ready(mut self: Pin<&mut Self>, lw: &LocalWaker) -> Poll<Result<(), Self::SinkError>> {
            Pin::new(&mut **self).poll_ready(lw)
        }

        fn start_send(mut self: Pin<&mut Self>, item: Self::SinkItem) -> Result<(), Self::SinkError> {
            Pin::new(&mut **self).start_send(item)
        }

        fn poll_flush(mut self: Pin<&mut Self>, lw: &LocalWaker) -> Poll<Result<(), Self::SinkError>> {
            Pin::new(&mut **self).poll_flush(lw)
        }

        fn poll_close(mut self: Pin<&mut Self>, lw: &LocalWaker) -> Poll<Result<(), Self::SinkError>> {
            Pin::new(&mut **self).poll_close(lw)
        }
    }
}

#[cfg(feature = "std")]
pub use self::if_std::*;

#[cfg(feature = "either")]
use either::Either;
#[cfg(feature = "either")]
impl<A, B> Sink for Either<A, B>
    where A: Sink,
          B: Sink<SinkItem=<A as Sink>::SinkItem,
                  SinkError=<A as Sink>::SinkError>
{
    type SinkItem = <A as Sink>::SinkItem;
    type SinkError = <A as Sink>::SinkError;

    fn poll_ready(self: Pin<&mut Self>, lw: &LocalWaker) -> Poll<Result<(), Self::SinkError>> {
        unsafe {
            match Pin::get_unchecked_mut(self) {
                Either::Left(x) => Pin::new_unchecked(x).poll_ready(lw),
                Either::Right(x) => Pin::new_unchecked(x).poll_ready(lw),
            }
        }
    }

    fn start_send(self: Pin<&mut Self>, item: Self::SinkItem) -> Result<(), Self::SinkError> {
        unsafe {
            match Pin::get_unchecked_mut(self) {
                Either::Left(x) => Pin::new_unchecked(x).start_send(item),
                Either::Right(x) => Pin::new_unchecked(x).start_send(item),
            }
        }
    }

    fn poll_flush(self: Pin<&mut Self>, lw: &LocalWaker) -> Poll<Result<(), Self::SinkError>> {
        unsafe {
            match Pin::get_unchecked_mut(self) {
                Either::Left(x) => Pin::new_unchecked(x).poll_flush(lw),
                Either::Right(x) => Pin::new_unchecked(x).poll_flush(lw),
            }
        }
    }

    fn poll_close(self: Pin<&mut Self>, lw: &LocalWaker) -> Poll<Result<(), Self::SinkError>> {
        unsafe {
            match Pin::get_unchecked_mut(self) {
                Either::Left(x) => Pin::new_unchecked(x).poll_close(lw),
                Either::Right(x) => Pin::new_unchecked(x).poll_close(lw),
            }
        }
    }
}
