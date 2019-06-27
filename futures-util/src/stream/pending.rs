use core::marker;
use core::pin::Pin;

use futures_core::{Stream, Poll};
use futures_core::task;

/// A stream which never returns any elements.
///
/// This stream can be created with the `stream::pending` function.
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct Pending<T> {
    _data: marker::PhantomData<T>,
}

/// Creates a stream which never returns any elements.
///
/// The returned stream will always return `Pending` when polled.
pub fn pending<T>() -> Pending<T> {
    Pending { _data: marker::PhantomData }
}

impl<T> Stream for Pending<T> {
    type Item = T;

    fn poll_next(self: Pin<&mut Self>, _: &mut task::Context<'_>) -> Poll<Option<Self::Item>> {
        Poll::Pending
    }
}
