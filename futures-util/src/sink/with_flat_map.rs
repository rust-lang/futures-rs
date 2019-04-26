use core::marker::PhantomData;
use core::pin::Pin;
use futures_core::stream::Stream;
use futures_core::task::{Context, Poll};
use futures_sink::Sink;
use pin_utils::{unsafe_pinned, unsafe_unpinned};

/// Sink for the [`with_flat_map`](super::SinkExt::with_flat_map) method.
#[derive(Debug)]
#[must_use = "sinks do nothing unless polled"]
pub struct WithFlatMap<Si, Item, U, St, F>
where
    Si: Sink<Item>,
    F: FnMut(U) -> St,
    St: Stream<Item = Result<Item, Si::SinkError>>,
{
    sink: Si,
    f: F,
    stream: Option<St>,
    buffer: Option<Item>,
    _marker: PhantomData<fn(U)>,
}

impl<Si, Item, U, St, F> Unpin for WithFlatMap<Si, Item, U, St, F>
where
    Si: Sink<Item> + Unpin,
    F: FnMut(U) -> St,
    St: Stream<Item = Result<Item, Si::SinkError>> + Unpin,
{}

impl<Si, Item, U, St, F> WithFlatMap<Si, Item, U, St, F>
where
    Si: Sink<Item>,
    F: FnMut(U) -> St,
    St: Stream<Item = Result<Item, Si::SinkError>>,
{
    unsafe_pinned!(sink: Si);
    unsafe_unpinned!(f: F);
    unsafe_pinned!(stream: Option<St>);

    pub(super) fn new(sink: Si, f: F) -> Self {
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
    pub fn get_pin_mut<'a>(self: Pin<&'a mut Self>) -> Pin<&'a mut Si> {
        self.sink()
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
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), Si::SinkError>> {
        let WithFlatMap { sink, stream, buffer, .. } =
            unsafe { Pin::get_unchecked_mut(self) };
        let mut sink = unsafe { Pin::new_unchecked(sink) };
        let mut stream = unsafe { Pin::new_unchecked(stream) };

        if buffer.is_some() {
            try_ready!(sink.as_mut().poll_ready(cx));
            let item = buffer.take().unwrap();
            try_ready!(Poll::Ready(sink.as_mut().start_send(item)));
        }
        if let Some(mut some_stream) = stream.as_mut().as_pin_mut() {
            while let Some(x) = ready!(some_stream.as_mut().poll_next(cx)) {
                let item = try_ready!(Poll::Ready(x));
                match sink.as_mut().poll_ready(cx)? {
                    Poll::Ready(()) => sink.as_mut().start_send(item)?,
                    Poll::Pending => {
                        *buffer = Some(item);
                        return Poll::Pending;
                    }
                };
            }
        }
        stream.set(None);
        Poll::Ready(Ok(()))
    }
}

impl<S, Item, U, St, F> Stream for WithFlatMap<S, Item, U, St, F>
where
    S: Stream + Sink<Item>,
    F: FnMut(U) -> St,
    St: Stream<Item = Result<Item, S::SinkError>>,
{
    type Item = S::Item;
    fn poll_next(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<S::Item>> {
        self.sink().poll_next(cx)
    }
}

impl<Si, Item, U, St, F> Sink<U> for WithFlatMap<Si, Item, U, St, F>
where
    Si: Sink<Item>,
    F: FnMut(U) -> St,
    St: Stream<Item = Result<Item, Si::SinkError>>,
{
    type SinkError = Si::SinkError;

    fn poll_ready(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), Self::SinkError>> {
        self.try_empty_stream(cx)
    }

    fn start_send(
        mut self: Pin<&mut Self>,
        item: U,
    ) -> Result<(), Self::SinkError> {
        assert!(self.stream.is_none());
        let stream = (self.as_mut().f())(item);
        self.stream().set(Some(stream));
        Ok(())
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), Self::SinkError>> {
        match self.as_mut().try_empty_stream(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Ok(())) => self.as_mut().sink().poll_flush(cx),
            Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
        }
    }

    fn poll_close(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), Self::SinkError>> {
        match self.as_mut().try_empty_stream(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Ok(())) => self.as_mut().sink().poll_close(cx),
            Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
        }
    }
}
