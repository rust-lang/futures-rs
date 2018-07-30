//! Asynchronous sinks
//!
//! This crate contains the `Sink` trait which allows values to be sent
//! asynchronously.

#![no_std]
#![warn(missing_docs, missing_debug_implementations)]
#![doc(html_root_url = "https://rust-lang-nursery.github.io/futures-doc/0.3.0-alpha.2/futures_sink")]

#![feature(pin, arbitrary_self_types, futures_api)]

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

use futures_core::task::{self, Poll};
use core::marker::Unpin;
use core::mem::PinMut;

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
    /// is registered to be notified (via `cx.waker()`) when `poll_ready`
    /// should be called again.
    ///
    /// In most cases, if the sink encounters an error, the sink will
    /// permanently be unable to receive items.
    fn poll_ready(self: PinMut<Self>, cx: &mut task::Context) -> Poll<Result<(), Self::SinkError>>;

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
    fn start_send(self: PinMut<Self>, item: Self::SinkItem)
                  -> Result<(), Self::SinkError>;

    /// Flush any remaining output from this sink.
    ///
    /// Returns `Ok(Poll::Ready(()))` when no buffered items remain. If this
    /// value is returned then it is guaranteed that all previous values sent
    /// via `start_send` have been flushed.
    ///
    /// Returns `Ok(Async::Pending)` if there is more work left to do, in which
    /// case the current task is scheduled (via `cx.waker()`) to wake up when
    /// `poll_flush` should be called again.
    ///
    /// In most cases, if the sink encounters an error, the sink will
    /// permanently be unable to receive items.
    fn poll_flush(self: PinMut<Self>, cx: &mut task::Context) -> Poll<Result<(), Self::SinkError>>;

    /// Flush any remaining output and close this sink, if necessary.
    ///
    /// Returns `Ok(Poll::Ready(()))` when no buffered items remain and the sink
    /// has been successfully closed.
    ///
    /// Returns `Ok(Async::Pending)` if there is more work left to do, in which
    /// case the current task is scheduled (via `cx.waker()`) to wake up when
    /// `poll_close` should be called again.
    ///
    /// If this function encounters an error, the sink should be considered to
    /// have failed permanently, and no more `Sink` methods should be called.
    fn poll_close(self: PinMut<Self>, cx: &mut task::Context) -> Poll<Result<(), Self::SinkError>>;
}

impl<'a, S: ?Sized + Sink + Unpin> Sink for &'a mut S {
    type SinkItem = S::SinkItem;
    type SinkError = S::SinkError;

    fn poll_ready(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<Result<(), Self::SinkError>> {
        PinMut::new(&mut **self).poll_ready(cx)
    }

    fn start_send(mut self: PinMut<Self>, item: Self::SinkItem) -> Result<(), Self::SinkError> {
        PinMut::new(&mut **self).start_send(item)
    }

    fn poll_flush(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<Result<(), Self::SinkError>> {
        PinMut::new(&mut **self).poll_flush(cx)
    }

    fn poll_close(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<Result<(), Self::SinkError>> {
        PinMut::new(&mut **self).poll_close(cx)
    }
}

impl<'a, S: ?Sized + Sink> Sink for PinMut<'a, S> {
    type SinkItem = S::SinkItem;
    type SinkError = S::SinkError;

    fn poll_ready(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<Result<(), Self::SinkError>> {
        S::poll_ready((*self).reborrow(), cx)
    }

    fn start_send(mut self: PinMut<Self>, item: Self::SinkItem) -> Result<(), Self::SinkError> {
        S::start_send((*self).reborrow(), item)
    }

    fn poll_flush(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<Result<(), Self::SinkError>> {
        S::poll_flush((*self).reborrow(), cx)
    }

    fn poll_close(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<Result<(), Self::SinkError>> {
        S::poll_close((*self).reborrow(), cx)
    }
}

if_std! {
    mod channel_impls;

    /// The error type for `Vec` and `VecDequeue` when used as `Sink`s.
    /// Values of this type can never be created.
    #[derive(Copy, Clone, Debug)]
    pub enum VecSinkError {}

    impl<T> Sink for ::std::vec::Vec<T> {
        type SinkItem = T;
        type SinkError = VecSinkError;

        fn poll_ready(self: PinMut<Self>, _: &mut task::Context) -> Poll<Result<(), Self::SinkError>> {
            Poll::Ready(Ok(()))
        }

        fn start_send(self: PinMut<Self>, item: Self::SinkItem) -> Result<(), Self::SinkError> {
            // TODO: impl<T> Unpin for Vec<T> {}
            unsafe { PinMut::get_mut_unchecked(self) }.push(item);
            Ok(())
        }

        fn poll_flush(self: PinMut<Self>, _: &mut task::Context) -> Poll<Result<(), Self::SinkError>> {
            Poll::Ready(Ok(()))
        }

        fn poll_close(self: PinMut<Self>, _: &mut task::Context) -> Poll<Result<(), Self::SinkError>> {
            Poll::Ready(Ok(()))
        }
    }

    impl<T> Sink for ::std::collections::VecDeque<T> {
        type SinkItem = T;
        type SinkError = VecSinkError;

        fn poll_ready(self: PinMut<Self>, _: &mut task::Context) -> Poll<Result<(), Self::SinkError>> {
            Poll::Ready(Ok(()))
        }

        fn start_send(self: PinMut<Self>, item: Self::SinkItem) -> Result<(), Self::SinkError> {
            // TODO: impl<T> Unpin for Vec<T> {}
            unsafe { PinMut::get_mut_unchecked(self) }.push_back(item);
            Ok(())
        }

        fn poll_flush(self: PinMut<Self>, _: &mut task::Context) -> Poll<Result<(), Self::SinkError>> {
            Poll::Ready(Ok(()))
        }

        fn poll_close(self: PinMut<Self>, _: &mut task::Context) -> Poll<Result<(), Self::SinkError>> {
            Poll::Ready(Ok(()))
        }
    }

    impl<S: ?Sized + Sink + Unpin> Sink for ::std::boxed::Box<S> {
        type SinkItem = S::SinkItem;
        type SinkError = S::SinkError;

        fn poll_ready(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<Result<(), Self::SinkError>> {
            PinMut::new(&mut **self).poll_ready(cx)
        }

        fn start_send(mut self: PinMut<Self>, item: Self::SinkItem) -> Result<(), Self::SinkError> {
            PinMut::new(&mut **self).start_send(item)
        }

        fn poll_flush(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<Result<(), Self::SinkError>> {
            PinMut::new(&mut **self).poll_flush(cx)
        }

        fn poll_close(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<Result<(), Self::SinkError>> {
            PinMut::new(&mut **self).poll_close(cx)
        }
    }
}

#[cfg(feature = "either")]
extern crate either;
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

    fn poll_ready(self: PinMut<Self>, cx: &mut task::Context) -> Poll<Result<(), Self::SinkError>> {
        unsafe {
            match PinMut::get_mut_unchecked(self) {
                Either::Left(x) => PinMut::new_unchecked(x).poll_ready(cx),
                Either::Right(x) => PinMut::new_unchecked(x).poll_ready(cx),
            }
        }
    }

    fn start_send(self: PinMut<Self>, item: Self::SinkItem) -> Result<(), Self::SinkError> {
        unsafe {
            match PinMut::get_mut_unchecked(self) {
                Either::Left(x) => PinMut::new_unchecked(x).start_send(item),
                Either::Right(x) => PinMut::new_unchecked(x).start_send(item),
            }
        }
    }

    fn poll_flush(self: PinMut<Self>, cx: &mut task::Context) -> Poll<Result<(), Self::SinkError>> {
        unsafe {
            match PinMut::get_mut_unchecked(self) {
                Either::Left(x) => PinMut::new_unchecked(x).poll_flush(cx),
                Either::Right(x) => PinMut::new_unchecked(x).poll_flush(cx),
            }
        }
    }

    fn poll_close(self: PinMut<Self>, cx: &mut task::Context) -> Poll<Result<(), Self::SinkError>> {
        unsafe {
            match PinMut::get_mut_unchecked(self) {
                Either::Left(x) => PinMut::new_unchecked(x).poll_close(cx),
                Either::Right(x) => PinMut::new_unchecked(x).poll_close(cx),
            }
        }
    }
}
