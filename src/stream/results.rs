use core::marker::PhantomData;

use {Poll, Async};
use stream::Stream;


/// A stream combinator used to convert a `Stream<Item=T,Error=E>`
/// to a `Stream<Item=Result<T,E>>`.
///
/// A poll on this stream will never return an `Err`. As such the
/// actual error type is parameterized, so it can match whatever error
/// type is needed.
///
/// This structure is produced by the `Sream::results` method.
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct Results<S: Stream, E> {
    inner: S,
    phantom: PhantomData<E>
}

pub fn new<S, E>(s: S) -> Results<S, E> where S: Stream {
    Results {
        inner: s,
        phantom: PhantomData
    }
}

impl<S: Stream, E> Stream for Results<S, E> {
    type Item = Result<S::Item, S::Error>;
    type Error = E;

    fn poll(&mut self) -> Poll<Option<Result<S::Item, S::Error>>, E> {
        match self.inner.poll() {
            Ok(Async::Ready(Some(item))) => Ok(Async::Ready(Some(Ok(item)))),
            Err(e) => Ok(Async::Ready(Some(Err(e)))),
            Ok(Async::Ready(None)) => Ok(Async::Ready(None)),
            Ok(Async::NotReady) => Ok(Async::NotReady)
        }
    }
}

