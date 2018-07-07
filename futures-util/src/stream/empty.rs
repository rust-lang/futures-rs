use core::marker::{Unpin, PhantomData};
use core::mem::PinMut;
use futures_core::stream::Stream;
use futures_core::task::{Context, Poll};

/// A stream which contains no elements.
///
/// This stream can be created with the `stream::empty` function.
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct Empty<T> {
    _phantom: PhantomData<T>
}

/// Creates a stream which contains no elements.
///
/// The returned stream will always return `Ready(None)` when polled.
pub fn empty<T>() -> Empty<T> {
    Empty {
        _phantom: PhantomData
    }
}

impl<T> Unpin for Empty<T> {}

impl<T> Stream for Empty<T> {
    type Item = T;

    fn poll_next(self: PinMut<Self>, _: &mut Context) -> Poll<Option<Self::Item>> {
        Poll::Ready(None)
    }
}
