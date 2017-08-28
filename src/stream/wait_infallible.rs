use {Stream, Never};
use stream::Wait;
use never::InfallibleResultExt;

/// A stream combinator which converts an asynchronous infallible stream to a
/// **blocking iterator**.
///
/// Created by the `Stream::wait_infallible` method, this function transforms an
/// infallible stream into a standard iterator. This is implemented by blocking the
/// current thread while items on the underlying stream aren't ready yet.
#[derive(Debug)]
#[must_use = "iterators do nothing unless advanced"]
pub struct WaitInfallible<S> {
    inner: Wait<S>
}

pub fn new<S>(stream: S) -> WaitInfallible<S> where S: Stream<Error=Never> {
    WaitInfallible {
        inner: stream.wait()
    }
}

impl<S> Iterator for WaitInfallible<S> where S: Stream<Error=Never> {
    type Item = S::Item;

    fn next(&mut self) -> Option<S::Item> {
        self.inner.next()
            .map(|r| r.infallible())
    }
}