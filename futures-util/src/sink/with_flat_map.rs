use core::marker::{Unpin, PhantomData};
use core::mem::PinMut;

use futures_core::{task, Poll, Stream};
use futures_sink::Sink;

/// Sink for the `Sink::with_flat_map` combinator, chaining a computation that returns an iterator
/// to run prior to pushing a value into the underlying sink
#[derive(Debug)]
#[must_use = "sinks do nothing unless polled"]
pub struct WithFlatMap<S, U, St, F>
where
    S: Sink,
    F: FnMut(U) -> St,
    St: Stream<Item = Result<S::SinkItem, S::SinkError>>,
{
    sink: S,
    f: F,
    stream: Option<St>,
    buffer: Option<S::SinkItem>,
    _phantom: PhantomData<fn(U)>,
}

impl<S, U, St, F> Unpin for WithFlatMap<S, U, St, F>
where
    S: Sink + Unpin,
    F: FnMut(U) -> St,
    St: Stream<Item = Result<S::SinkItem, S::SinkError>> + Unpin,
{}

pub fn new<S, U, St, F>(sink: S, f: F) -> WithFlatMap<S, U, St, F>
where
    S: Sink,
    F: FnMut(U) -> St,
    St: Stream<Item = Result<S::SinkItem, S::SinkError>>,
{
    WithFlatMap {
        sink: sink,
        f: f,
        stream: None,
        buffer: None,
        _phantom: PhantomData,
    }
}

impl<S, U, St, F> WithFlatMap<S, U, St, F>
where
    S: Sink,
    F: FnMut(U) -> St,
    St: Stream<Item = Result<S::SinkItem, S::SinkError>>,
{
    unsafe_pinned!(sink -> S);
    unsafe_unpinned!(f -> F);
    unsafe_pinned!(stream -> Option<St>);

    /// Get a shared reference to the inner sink.
    pub fn get_ref(&self) -> &S {
        &self.sink
    }

    /// Get a mutable reference to the inner sink.
    pub fn get_mut(&mut self) -> &mut S {
        &mut self.sink
    }

    /// Get a pinned mutable reference to the inner sink.
    pub fn get_pin_mut<'a>(self: PinMut<'a, Self>) -> PinMut<'a, S> {
        unsafe { PinMut::new_unchecked(&mut PinMut::get_mut_unchecked(self).sink) }
    }

    /// Consumes this combinator, returning the underlying sink.
    ///
    /// Note that this may discard intermediate state of this combinator, so
    /// care should be taken to avoid losing resources when this is called.
    pub fn into_inner(self) -> S {
        self.sink
    }

    fn try_empty_stream(self: PinMut<Self>, cx: &mut task::Context) -> Poll<Result<(), S::SinkError>> {
        let WithFlatMap {
            sink,
            f: _,
            stream,
            buffer,
            _phantom,
        } = unsafe { PinMut::get_mut_unchecked(self) };
        let mut sink = unsafe { PinMut::new_unchecked(sink) };
        let mut stream = unsafe { PinMut::new_unchecked(stream) };


        if let Some(x) = buffer.take() {
            match try_poll!(sink.reborrow().poll_ready(cx)) {
                Poll::Ready(()) => try_ready!(Poll::Ready(sink.reborrow().start_send(x))),
                Poll::Pending => {
                    *buffer = Some(x);
                    return Poll::Pending;
                }
            };
        }
        if let Some(mut some_stream) = stream.reborrow().as_pin_mut() {
            while let Some(x) = ready!(some_stream.reborrow().poll_next(cx)) {
                let x = try_ready!(Poll::Ready(x));
                match try_poll!(sink.reborrow().poll_ready(cx)) {
                    Poll::Ready(()) => try_poll!(Poll::Ready(sink.reborrow().start_send(x))),
                    Poll::Pending => {
                        *buffer = Some(x);
                        return Poll::Pending;
                    }
                };
            }
        }
        PinMut::set(stream, None);
        Poll::Ready(Ok(()))
    }
}

impl<S, U, St, F> Stream for WithFlatMap<S, U, St, F>
where
    S: Stream + Sink,
    F: FnMut(U) -> St,
    St: Stream<Item = Result<S::SinkItem, S::SinkError>>,
{
    type Item = S::Item;
    fn poll_next(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<Option<S::Item>> {
        self.sink().poll_next(cx)
    }
}

impl<S, U, St, F> Sink for WithFlatMap<S, U, St, F>
where
    S: Sink,
    F: FnMut(U) -> St,
    St: Stream<Item = Result<S::SinkItem, S::SinkError>>,
{
    type SinkItem = U;
    type SinkError = S::SinkError;

    fn poll_ready(self: PinMut<Self>, cx: &mut task::Context) -> Poll<Result<(), Self::SinkError>> {
        self.try_empty_stream(cx)
    }

    fn start_send(mut self: PinMut<Self>, i: Self::SinkItem) -> Result<(), Self::SinkError> {
        assert!(self.stream().is_none());
        let stream = (self.f())(i);
        PinMut::set(self.stream(), Some(stream));
        Ok(())
    }

    fn poll_flush(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<Result<(), Self::SinkError>> {
        match self.reborrow().try_empty_stream(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Ok(())) => self.sink().poll_flush(cx),
            Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
        }
    }

    fn poll_close(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<Result<(), Self::SinkError>> {
        match self.reborrow().try_empty_stream(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Ok(())) => self.sink().poll_close(cx),
            Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
        }
    }
}
