use core::marker::Unpin;
use core::pin::Pin;
use futures_core::stream::Stream;
use futures_core::task::{LocalWaker, Poll};
use futures_sink::{Sink};
use pin_utils::{unsafe_pinned, unsafe_unpinned};

/// Sink for the `Sink::sink_map_err` combinator.
#[derive(Debug)]
#[must_use = "sinks do nothing unless polled"]
pub struct SinkMapErr<Si, F> {
    sink: Si,
    f: Option<F>,
}

impl<Si: Unpin, F> Unpin for SinkMapErr<Si, F> {}

impl<Si, F> SinkMapErr<Si, F> {
    unsafe_pinned!(sink: Si);
    unsafe_unpinned!(f: Option<F>);

    pub(super) fn new(sink: Si, f: F) -> SinkMapErr<Si, F> {
        SinkMapErr { sink, f: Some(f) }
    }

    /// Get a shared reference to the inner sink.
    pub fn get_ref(&self) -> &Si {
        &self.sink
    }

    /// Get a mutable reference to the inner sink.
    pub fn get_mut(&mut self) -> &mut Si {
        &mut self.sink
    }

    /// Get a pinned reference to the inner sink.
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

    fn take_f(self: Pin<&mut Self>) -> F {
        self.f().take().expect("polled MapErr after completion")
    }
}

impl<Si, F, E> Sink for SinkMapErr<Si, F>
    where Si: Sink,
          F: FnOnce(Si::SinkError) -> E,
{
    type SinkItem = Si::SinkItem;
    type SinkError = E;

    fn poll_ready(
        mut self: Pin<&mut Self>,
        lw: &LocalWaker,
    ) -> Poll<Result<(), Self::SinkError>> {
        #[allow(clippy::redundant_closure)] // https://github.com/rust-lang-nursery/rust-clippy/issues/1439
        self.as_mut().sink().poll_ready(lw).map_err(|e| self.as_mut().take_f()(e))
    }

    fn start_send(
        mut self: Pin<&mut Self>,
        item: Self::SinkItem,
    ) -> Result<(), Self::SinkError> {
        #[allow(clippy::redundant_closure)] // https://github.com/rust-lang-nursery/rust-clippy/issues/1439
        self.as_mut().sink().start_send(item).map_err(|e| self.as_mut().take_f()(e))
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        lw: &LocalWaker,
    ) -> Poll<Result<(), Self::SinkError>> {
        #[allow(clippy::redundant_closure)] // https://github.com/rust-lang-nursery/rust-clippy/issues/1439
        self.as_mut().sink().poll_flush(lw).map_err(|e| self.as_mut().take_f()(e))
    }

    fn poll_close(
        mut self: Pin<&mut Self>,
        lw: &LocalWaker,
    ) -> Poll<Result<(), Self::SinkError>> {
        #[allow(clippy::redundant_closure)] // https://github.com/rust-lang-nursery/rust-clippy/issues/1439
        self.as_mut().sink().poll_close(lw).map_err(|e| self.as_mut().take_f()(e))
    }
}

impl<S: Stream, F> Stream for SinkMapErr<S, F> {
    type Item = S::Item;

    fn poll_next(
        self: Pin<&mut Self>,
        lw: &LocalWaker,
    ) -> Poll<Option<S::Item>> {
        self.sink().poll_next(lw)
    }
}
