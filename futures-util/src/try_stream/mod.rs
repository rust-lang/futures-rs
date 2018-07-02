//! Streams
//!
//! This module contains a number of functions for working with `Streams`s
//! that return `Result`s, allowing for short-circuiting computations.

use futures_core::TryStream;

if_std! {
    // combinators
    mod try_collect;
    pub use self::try_collect::TryCollect;
}

impl<S: TryStream> TryStreamExt for S {}

/// Adapters specific to `Result`-returning streams
pub trait TryStreamExt: TryStream {
    /// Attempt to Collect all of the values of this stream into a vector, returning a
    /// future representing the result of that computation.
    ///
    /// This combinator will collect all successful results of this stream and
    /// collect them into a `Vec<Self::Item>`. If an error happens then all
    /// collected elements will be dropped and the error will be returned.
    ///
    /// The returned future will be resolved when the stream terminates.
    ///
    /// This method is only available when the `std` feature of this
    /// library is activated, and it is activated by default.
    ///
    /// # Examples
    ///
    /// ```
    /// # extern crate futures;
    /// # extern crate futures_channel;
    /// use std::thread;
    ///
    /// use futures::prelude::*;
    /// use futures_channel::mpsc;
    /// use futures::executor::block_on;
    ///
    /// let (mut tx, rx) = mpsc::unbounded();
    ///
    /// thread::spawn(move || {
    ///     for i in (1..=5) {
    ///         tx.unbounded_send(Ok(i)).unwrap();
    ///     }
    ///     tx.unbounded_send(Err(6)).unwrap();
    /// });
    ///
    /// let output: Result<Vec<i32>, i32> = block_on(rx.try_collect());
    /// assert_eq!(output, Err(6));
    /// ```
    #[cfg(feature = "std")]
    fn try_collect<C: Default + Extend<Self::TryItem>>(self) -> TryCollect<Self, C>
        where Self: Sized
    {
        try_collect::new(self)
    }
}
