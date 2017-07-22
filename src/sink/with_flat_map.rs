use core::marker::PhantomData;

use {Poll, Async, StartSend, AsyncSink};
use sink::Sink;
use stream::Stream;

/// Sink for the `Sink::with_flat_map` combinator, chaining a computation that returns an iterator
/// to run prior to pushing a value into the underlying sink
#[derive(Debug)]
#[must_use = "sinks do nothing unless polled"]
pub struct WithFlatMap<S, U, F, I>
where
    S: Sink,
    F: FnMut(U) -> I,
    I: IntoIterator<Item = S::SinkItem>,
{
    sink: S,
    f: F,
    iter: Option<I::IntoIter>,
    buffer: Option<S::SinkItem>,
    _phantom: PhantomData<fn(U)>,
}

pub fn new<S, U, F, I>(sink: S, f: F) -> WithFlatMap<S, U, F, I>
where
    S: Sink,
    F: FnMut(U) -> I,
    I: IntoIterator<Item = S::SinkItem>,
{
    WithFlatMap {
        sink: sink,
        f: f,
        iter: None,
        buffer: None,
        _phantom: PhantomData
    }
}

impl<S, U, F, I> WithFlatMap<S, U, F, I>
where
    S: Sink,
    F: FnMut(U) -> I,
    I: IntoIterator<Item = S::SinkItem>,
{
    /// Get a shared reference to the inner sink.
    pub fn get_ref(&self) -> &S {
        &self.sink
    }

    /// Get a mutable reference to the inner sink.
    pub fn get_mut(&mut self) -> &mut S {
        &mut self.sink
    }

    /// Consumes this combinator, returning the underlying sink.
    ///
    /// Note that this may discard intermediate state of this combinator, so
    /// care should be taken to avoid losing resources when this is called.
    pub fn into_inner(self) -> S {
        self.sink
    }

    fn try_empty_iter(&mut self) -> Poll<(), S::SinkError> {
        if let Some(x) = self.buffer.take() {
            if let AsyncSink::NotReady(x) = try!(self.sink.start_send(x)) {
                self.buffer = Some(x);
                return Ok(Async::NotReady);
            }
        }
        if let Some(mut iter) = self.iter.take() {
            while let Some(x) = iter.next() {
                if let AsyncSink::NotReady(x) = try!(self.sink.start_send(x)) {
                    self.iter = Some(iter);
                    self.buffer = Some(x);
                    return Ok(Async::NotReady);
                }
            }
        }
        Ok(Async::Ready(()))
    }
}

impl<S, U, F, I> Stream for WithFlatMap<S, U, F, I>
where
    S: Stream + Sink,
    F: FnMut(U) -> I,
    I: IntoIterator<Item = S::SinkItem>,
{
    type Item = S::Item;
    type Error = S::Error;
    fn poll(&mut self) -> Poll<Option<S::Item>, S::Error> {
        self.sink.poll()
    }
}

impl<S, U, F, I> Sink for WithFlatMap<S, U, F, I>
where
    S: Sink,
    F: FnMut(U) -> I,
    I: IntoIterator<Item = S::SinkItem>,
{
    type SinkItem = U;
    type SinkError = S::SinkError;
    fn start_send(&mut self, i: Self::SinkItem) -> StartSend<Self::SinkItem, Self::SinkError> {
        if try!(self.try_empty_iter()).is_not_ready() {
            return Ok(AsyncSink::NotReady(i));
        }
        assert!(self.iter.is_none());
        self.iter = Some((self.f)(i).into_iter());
        try!(self.try_empty_iter());
        Ok(AsyncSink::Ready)
    }
    fn poll_complete(&mut self) -> Poll<(), Self::SinkError> {
        if try!(self.try_empty_iter()).is_not_ready() {
            return Ok(Async::NotReady);
        }
        self.sink.poll_complete()
    }
    fn close(&mut self) -> Poll<(), Self::SinkError> {
        if try!(self.try_empty_iter()).is_not_ready() {
            return Ok(Async::NotReady);
        }
        assert!(self.iter.is_none());
        self.sink.close()
    }
}
