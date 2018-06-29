#![no_std]
#![cfg_attr(feature = "nightly", feature(arbitrary_self_types))]
#![cfg_attr(feature = "nightly", feature(pin))]

macro_rules! if_nightly {
    ($($i:item)*) => ($(
        #[cfg(feature = "nightly")]
        $i
    )*)
}

if_nightly! {
    macro_rules! if_std {
        ($($i:item)*) => ($(
            #[cfg(feature = "std")]
            $i
        )*)
    }

    extern crate futures_core;
    extern crate futures_executor;

    use core::mem::PinMut;
    use futures_core::{Future, Stream, Poll, task};

    if_std! {
        extern crate std;

        mod executor;
        mod unsafe_pin;

        use std::boxed::PinBox;

        pub use executor::{StableExecutor, block_on_stable};
        use unsafe_pin::UnsafePin;
    }

    /// A trait for `Future`s which can be pinned to a particular location in memory.
    ///
    /// These futures take `self` by `PinMut<Self>`, rather than `&mut Self`.
    /// This allows types which are not [`Unpin`](::std::marker::Unpin) to guarantee
    /// that they won't be moved after being polled. Since they won't be moved, it's
    /// possible for them to safely contain references to themselves.
    ///
    /// The most common examples of such self-referential `StableFuture`s are `#[async]`
    /// functions and `async_block!`s.
    ///
    /// All types which implement `Future` also implement `StableFuture` automatically.
    pub trait StableFuture {
        /// A successful value
        type Item;

        /// An error
        type Error;

        /// Attempt to resolve the future to a final value, registering the current task
        /// for wakeup if the value is not yet available.
        ///
        /// This method takes `self` by `PinMut`, and so calling it requires putting `Self`
        /// in a [`PinBox`](::std::boxed::PinBox) using the `pin` method, or otherwise
        /// guaranteeing that the location of `self` will not change after a call to `poll`.
        fn poll(self: PinMut<Self>, ctx: &mut task::Context) -> Poll<Self::Item, Self::Error>;

        /// Pin the future to a particular location by placing it on the heap.
        #[cfg(feature = "std")]
        fn pin<'a>(self) -> PinBox<Future<Item = Self::Item, Error = Self::Error> + Send + 'a>
            where Self: Send + Sized + 'a
        {
            PinBox::new(unsafe { UnsafePin::new(self) })
        }

        /// Pin the future to a particular location by placing it on the heap.
        ///
        /// This method is the same as `pin`, but doesn't require that `Self` can be
        /// safely sent across threads. `pin` should be preferred where possible.
        #[cfg(feature = "std")]
        fn pin_local<'a>(self) -> PinBox<Future<Item = Self::Item, Error = Self::Error> + 'a>
            where Self: Sized + 'a
        {
            PinBox::new(unsafe { UnsafePin::new(self) })
        }
    }

    impl<F: Future> StableFuture for F {
        type Item = F::Item;
        type Error = F::Error;

        fn poll(self: PinMut<Self>, ctx: &mut task::Context) -> Poll<Self::Item, Self::Error> {
            F::poll(unsafe { PinMut::get_mut_unchecked(self) }, ctx)
        }
    }

    /// A trait for `Stream`s which can be pinned to a particular location in memory.
    ///
    /// These streams take `self` by `PinMut<Self>`, rather than `&mut Self`.
    /// This allows types which are not [`Unpin`](::std::marker::Unpin) to guarantee
    /// that they won't be moved after being polled. Since they won't be moved, it's
    /// possible for them to safely contain references to themselves.
    ///
    /// The most common examples of such self-referential `StableStream`s are
    /// `#[async_stream(item = Foo)]` functions.
    ///
    /// All types which implement `Stream` also implement `StableStream` automatically.
    pub trait StableStream {
        /// A successful value
        type Item;
        /// An error
        type Error;

        /// Attempt to resolve the stream to the next value, registering the current task
        /// for wakeup if the value is not yet available.
        ///
        /// This method takes `self` by `PinMut`, and so calling it requires putting `Self`
        /// in a [`PinBox`](::std::boxed::PinBox) using the `pin` method, or otherwise
        /// guaranteeing that the location of `self` will not change after a call to `poll`.
        fn poll_next(self: PinMut<Self>, ctx: &mut task::Context) -> Poll<Option<Self::Item>, Self::Error>;

        /// Pin the stream to a particular location by placing it on the heap.
        #[cfg(feature = "std")]
        fn pin<'a>(self) -> PinBox<Stream<Item = Self::Item, Error = Self::Error> + Send + 'a>
            where Self: Send + Sized + 'a
        {
            PinBox::new(unsafe { UnsafePin::new(self) })
        }

        /// Pin the stream to a particular location by placing it on the heap.
        ///
        /// This method is the same as `pin`, but doesn't require that `Self` can be
        /// safely sent across threads. `pin` should be preferred where possible.
        #[cfg(feature = "std")]
        fn pin_local<'a>(self) -> PinBox<Stream<Item = Self::Item, Error = Self::Error> + 'a>
            where Self: Sized + 'a
        {
            PinBox::new(unsafe { UnsafePin::new(self) })
        }
    }

    impl<S: Stream> StableStream for S {
        type Item = S::Item;
        type Error = S::Error;

        fn poll_next(self: PinMut<Self>, ctx: &mut task::Context) -> Poll<Option<Self::Item>, Self::Error> {
            S::poll_next(unsafe { PinMut::get_mut_unchecked(self) }, ctx)
        }
    }
}
