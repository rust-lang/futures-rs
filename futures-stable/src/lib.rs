#![no_std]
#![cfg_attr(feature = "nightly", feature(arbitrary_self_types))]

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

    extern crate pin_api;
    extern crate futures_core;
    extern crate futures_executor;

    use pin_api::PinMut;
    use futures_core::{Future, Stream, Poll, task};

    if_std! {
        mod executor;
        mod unsafe_pin;

        use pin_api::PinBox;

        pub use executor::{StableExecutor, block_on_stable};
        use unsafe_pin::UnsafePin;
    }

    pub trait StableFuture {
        type Item;
        type Error;

        fn poll(self: PinMut<Self>, ctx: &mut task::Context) -> Poll<Self::Item, Self::Error>;

        #[cfg(feature = "std")]
        fn pin<'a>(self) -> PinBox<Future<Item = Self::Item, Error = Self::Error> + Send + 'a>
            where Self: Send + Sized + 'a
        {
            PinBox::new(unsafe { UnsafePin::new(self) })
        }

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

        fn poll(mut self: PinMut<Self>, ctx: &mut task::Context) -> Poll<Self::Item, Self::Error> {
            F::poll(unsafe { PinMut::get_mut(&mut self) }, ctx)
        }
    }

    pub trait StableStream {
        type Item;
        type Error;

        fn poll_next(self: PinMut<Self>, ctx: &mut task::Context) -> Poll<Option<Self::Item>, Self::Error>;

        #[cfg(feature = "std")]
        fn pin<'a>(self) -> PinBox<Stream<Item = Self::Item, Error = Self::Error> + Send + 'a>
            where Self: Send + Sized + 'a
        {
            PinBox::new(unsafe { UnsafePin::new(self) })
        }

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

        fn poll_next(mut self: PinMut<Self>, ctx: &mut task::Context) -> Poll<Option<Self::Item>, Self::Error> {
            S::poll_next(unsafe { PinMut::get_mut(&mut self) }, ctx)
        }
    }
}
