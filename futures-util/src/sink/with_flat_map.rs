use core::marker::{Unpin, PhantomData};
use core::pin::Pin;
use futures_core::stream::Stream;
use futures_core::task::{LocalWaker, Poll};
use futures_sink::Sink;
use pin_utils::{unsafe_pinned, unsafe_unpinned};

/// Sink for the `Sink::with_flat_map` combinator, chaining a computation that
/// returns an iterator to run prior to pushing a value into the underlying
/// sink.
#[derive(Debug)]
#[must_use = "sinks do nothing unless polled"]
pub struct WithFlatMap<Si, U, St, F>
where
    Si: Sink,
    F: FnMut(U) -> St,
    St: Stream<Item = Result<Si::SinkItem, Si::SinkError>>,
{
    sink: Si,
    f: F,
    stream: Option<St>,
    buffer: Option<Si::SinkItem>,
    _marker: PhantomData<fn(U)>,
}

impl<Si, U, St, F> Unpin for WithFlatMap<Si, U, St, F>
where
    Si: Sink + Unpin,
    F: FnMut(U) -> St,
    St: Stream<Item = Result<Si::SinkItem, Si::SinkError>> + Unpin,
{}

impl<Si, U, St, F> WithFlatMap<Si, U, St, F>
where
    Si: Sink,
    F: FnMut(U) -> St,
    St: Stream<Item = Result<Si::SinkItem, Si::SinkError>>,
{
    unsafe_pinned!(sink: Si);
    unsafe_unpinned!(f: F);
    unsafe_pinned!(stream: Option<St>);

    pub(super) fn new(sink: Si, f: F) -> WithFlatMap<Si, U, St, F> {
        WithFlatMap {
            sink,
            f,
            stream: None,
            buffer: None,
            _marker: PhantomData,
        }
    }

    /// Get a shared reference to the inner sink.
    pub fn get_ref(&self) -> &Si {
        &self.sink
    }

    /// Get a mutable reference to the inner sink.
    pub fn get_mut(&mut self) -> &mut Si {
        &mut self.sink
    }

    /// Get a pinned mutable reference to the inner sink.
    #[allow(clippy::needless_lifetimes)] // https://github.com/rust-lang/rust/issues/52675
    pub fn get_pin_mut<'a>(self: Pin<&'a mut Self>) -> Pin<&'a mut Si> {
        unsafe { Pin::map_unchecked_mut(self, |x| &mut x.sink) }
    }

    /// Consumes this combinator, returning the underlying sink.
    ///
    /// Note that this may discard intermediate state of this combinator, so
    /// care should be taken to avoid losing resources when this is called.
    pub fn into_inner(self) -> Si {
        self.sink
    }

    fn try_empty_stream(
        self: Pin<&mut Self>,
        lw: &LocalWaker,
    ) -> Poll<Result<(), Si::SinkError>> {
        let WithFlatMap { sink, stream, buffer, .. } =
            unsafe { Pin::get_unchecked_mut(self) };
        let mut sink = unsafe { Pin::new_unchecked(sink) };
        let mut stream = unsafe { Pin::new_unchecked(stream) };

        if buffer.is_some() {
            try_ready!(sink.as_mut().poll_ready(lw));
            let item = buffer.take().unwrap();
            try_ready!(Poll::Ready(sink.as_mut().start_send(item)));
        }
        if let Some(mut some_stream) = stream.as_mut().as_pin_mut() {
            while let Some(x) = ready!(some_stream.as_mut().poll_next(lw)) {
                let item = try_ready!(Poll::Ready(x));
                match try_poll!(sink.as_mut().poll_ready(lw)) {
                    Poll::Ready(()) => {
                        try_poll!(Poll::Ready(sink.as_mut().start_send(item)))
                    }
                    Poll::Pending => {
                        *buffer = Some(item);
                        return Poll::Pending;
                    }
                };
            }
        }
        Pin::set(&mut stream, None);
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
    fn poll_next(
        self: Pin<&mut Self>,
        lw: &LocalWaker,
    ) -> Poll<Option<S::Item>> {
        self.sink().poll_next(lw)
    }
}

impl<Si, U, St, F> Sink for WithFlatMap<Si, U, St, F>
where
    Si: Sink,
    F: FnMut(U) -> St,
    St: Stream<Item = Result<Si::SinkItem, Si::SinkError>>,
{
    type SinkItem = U;
    type SinkError = Si::SinkError;

    fn poll_ready(
        self: Pin<&mut Self>,
        lw: &LocalWaker,
    ) -> Poll<Result<(), Self::SinkError>> {
        self.try_empty_stream(lw)
    }

    fn start_send(
        mut self: Pin<&mut Self>,
        item: Self::SinkItem,
    ) -> Result<(), Self::SinkError> {
        assert!(self.stream.is_none());
        let stream = (self.as_mut().f())(item);
        self.stream().set(Some(stream));
        Ok(())
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        lw: &LocalWaker,
    ) -> Poll<Result<(), Self::SinkError>> {
        match self.as_mut().try_empty_stream(lw) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Ok(())) => self.as_mut().sink().poll_flush(lw),
            Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
        }
    }

    fn poll_close(
        mut self: Pin<&mut Self>,
        lw: &LocalWaker,
    ) -> Poll<Result<(), Self::SinkError>> {
        match self.as_mut().try_empty_stream(lw) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Ok(())) => self.as_mut().sink().poll_close(lw),
            Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
        }
    }
}
