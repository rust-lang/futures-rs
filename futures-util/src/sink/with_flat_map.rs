use core::marker::{Unpin, PhantomData};
use core::pin::PinMut;
use futures_core::stream::Stream;
use futures_core::task::{self, Poll};
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
    #[allow(needless_lifetimes)] // https://github.com/rust-lang/rust/issues/52675
    pub fn get_pin_mut<'a>(self: PinMut<'a, Self>) -> PinMut<'a, Si> {
        unsafe { PinMut::map_unchecked(self, |x| &mut x.sink) }
    }

    /// Consumes this combinator, returning the underlying sink.
    ///
    /// Note that this may discard intermediate state of this combinator, so
    /// care should be taken to avoid losing resources when this is called.
    pub fn into_inner(self) -> Si {
        self.sink
    }

    fn try_empty_stream(
        self: PinMut<Self>,
        cx: &mut task::Context,
    ) -> Poll<Result<(), Si::SinkError>> {
        let WithFlatMap { sink, stream, buffer, .. } =
            unsafe { PinMut::get_mut_unchecked(self) };
        let mut sink = unsafe { PinMut::new_unchecked(sink) };
        let mut stream = unsafe { PinMut::new_unchecked(stream) };

        if buffer.is_some() {
            try_ready!(sink.reborrow().poll_ready(cx));
            let item = buffer.take().unwrap();
            try_ready!(Poll::Ready(sink.reborrow().start_send(item)));
        }
        if let Some(mut some_stream) = stream.reborrow().as_pin_mut() {
            while let Some(x) = ready!(some_stream.reborrow().poll_next(cx)) {
                let item = try_ready!(Poll::Ready(x));
                match try_poll!(sink.reborrow().poll_ready(cx)) {
                    Poll::Ready(()) => {
                        try_poll!(Poll::Ready(sink.reborrow().start_send(item)))
                    }
                    Poll::Pending => {
                        *buffer = Some(item);
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
    fn poll_next(
        mut self: PinMut<Self>,
        cx: &mut task::Context,
    ) -> Poll<Option<S::Item>> {
        self.sink().poll_next(cx)
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
        self: PinMut<Self>,
        cx: &mut task::Context,
    ) -> Poll<Result<(), Self::SinkError>> {
        self.try_empty_stream(cx)
    }

    fn start_send(
        mut self: PinMut<Self>,
        item: Self::SinkItem,
    ) -> Result<(), Self::SinkError> {
        assert!(self.stream().is_none());
        let stream = (self.f())(item);
        PinMut::set(self.stream(), Some(stream));
        Ok(())
    }

    fn poll_flush(
        mut self: PinMut<Self>,
        cx: &mut task::Context,
    ) -> Poll<Result<(), Self::SinkError>> {
        match self.reborrow().try_empty_stream(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Ok(())) => self.sink().poll_flush(cx),
            Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
        }
    }

    fn poll_close(
        mut self: PinMut<Self>,
        cx: &mut task::Context,
    ) -> Poll<Result<(), Self::SinkError>> {
        match self.reborrow().try_empty_stream(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Ok(())) => self.sink().poll_close(cx),
            Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
        }
    }
}
