use core::fmt;
use core::marker::PhantomData;
use core::pin::Pin;
use futures_core::stream::{Stream, FusedStream};
use futures_core::task::{Context, Poll};
use futures_sink::Sink;
use pin_project::{pin_project, project};

/// Sink for the [`with_flat_map`](super::SinkExt::with_flat_map) method.
#[pin_project]
#[must_use = "sinks do nothing unless polled"]
pub struct WithFlatMap<Si, Item, U, St, F> {
    #[pin]
    sink: Si,
    f: F,
    #[pin]
    stream: Option<St>,
    buffer: Option<Item>,
    _marker: PhantomData<fn(U)>,
}

impl<Si, Item, U, St, F> fmt::Debug for WithFlatMap<Si, Item, U, St, F>
where
    Si: fmt::Debug,
    St: fmt::Debug,
    Item: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WithFlatMap")
            .field("sink", &self.sink)
            .field("stream", &self.stream)
            .field("buffer", &self.buffer)
            .finish()
    }
}

impl<Si, Item, U, St, F> WithFlatMap<Si, Item, U, St, F>
where
    Si: Sink<Item>,
    F: FnMut(U) -> St,
    St: Stream<Item = Result<Item, Si::Error>>,
{
    pub(super) fn new(sink: Si, f: F) -> Self {
        WithFlatMap {
            sink,
            f,
            stream: None,
            buffer: None,
            _marker: PhantomData,
        }
    }

    delegate_access_inner!(sink, Si, ());

    #[project]
    fn try_empty_stream(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), Si::Error>> {
        #[project]
        let WithFlatMap { mut sink, mut stream, buffer, .. } = self.project();

        if buffer.is_some() {
            ready!(sink.as_mut().poll_ready(cx))?;
            let item = buffer.take().unwrap();
            sink.as_mut().start_send(item)?;
        }
        if let Some(mut some_stream) = stream.as_mut().as_pin_mut() {
            while let Some(item) = ready!(some_stream.as_mut().poll_next(cx)?) {
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

// Forwarding impl of Stream from the underlying sink
impl<S, Item, U, St, F> Stream for WithFlatMap<S, Item, U, St, F>
where
    S: Stream + Sink<Item>,
    F: FnMut(U) -> St,
    St: Stream<Item = Result<Item, S::Error>>,
{
    type Item = S::Item;

    delegate_stream!(sink);
}

impl<S, Item, U, St, F> FusedStream for WithFlatMap<S, Item, U, St, F>
where
    S: FusedStream + Sink<Item>,
    F: FnMut(U) -> St,
    St: Stream<Item = Result<Item, S::Error>>,
{
    fn is_terminated(&self) -> bool {
        self.sink.is_terminated()
    }
}

impl<Si, Item, U, St, F> Sink<U> for WithFlatMap<Si, Item, U, St, F>
where
    Si: Sink<Item>,
    F: FnMut(U) -> St,
    St: Stream<Item = Result<Item, Si::Error>>,
{
    type Error = Si::Error;

    fn poll_ready(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        self.try_empty_stream(cx)
    }

    #[project]
    fn start_send(
        self: Pin<&mut Self>,
        item: U,
    ) -> Result<(), Self::Error> {
        #[project]
        let WithFlatMap { mut stream, f, .. } = self.project();

        assert!(stream.is_none());
        stream.set(Some(f(item)));
        Ok(())
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        ready!(self.as_mut().try_empty_stream(cx)?);
        self.project().sink.poll_flush(cx)
    }

    fn poll_close(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        ready!(self.as_mut().try_empty_stream(cx)?);
        self.project().sink.poll_close(cx)
    }
}
