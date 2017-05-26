use core::marker;

use stream::Stream;
use {Poll, Async};

/// A stream which contains no elements.
///
/// This stream can be created with the `stream::empty` function.
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct Empty<T, E> {
    used: bool,
    _data: marker::PhantomData<(T, E)>,
}

/// Creates a stream which contains no elements.
///
/// The returned stream will return `Ready(None)` when polled.
pub fn empty<T, E>() -> Empty<T, E> {
    Empty {
        used: false,
        _data: marker::PhantomData
    }
}

impl<T, E> Stream for Empty<T, E> {
    type Item = T;
    type Error = E;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        if self.used {
            panic!("cannot poll after eof");
        }
        self.used = true;
        Ok(Async::Ready(None))
    }
}
