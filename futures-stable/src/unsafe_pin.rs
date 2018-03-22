use core::mem::Pin;
use futures_core::{Future, Stream, Poll, task};

use {StableFuture, StableStream};

pub(crate) struct UnsafePin<T> {
    inner: T,
}

impl<T> UnsafePin<T> {
    pub(crate) unsafe fn new(inner: T) -> UnsafePin<T> {
        UnsafePin { inner }
    }
}

impl<'a, T: StableFuture> Future for UnsafePin<T> {
    type Item = T::Item;
    type Error = T::Error;
    fn poll(&mut self, ctx: &mut task::Context) -> Poll<Self::Item, Self::Error> {
        T::poll(unsafe { Pin::new_unchecked(&mut self.inner) }, ctx)
    }
}

impl<'a, T: StableStream> Stream for UnsafePin<T> {
    type Item = T::Item;
    type Error = T::Error;
    fn poll_next(&mut self, ctx: &mut task::Context) -> Poll<Option<Self::Item>, Self::Error> {
        T::poll_next(unsafe { Pin::new_unchecked(&mut self.inner) }, ctx)
    }
}
