use core::marker::{Unpin, PhantomData};
use core::mem::{self, PinMut};
use futures_core::future::Future;
use futures_core::stream::Stream;
use futures_core::task::{Poll, Context};
use futures_sink::Sink;

/// Sink for the `Sink::with` combinator, chaining a computation to run *prior*
/// to pushing a value into the underlying sink.
#[derive(Debug)]
#[must_use = "sinks do nothing unless polled"]
pub struct With<S, U, Fut, F>
    where S: Sink,
          F: FnMut(U) -> Fut,
          Fut: Future,
{
    sink: S,
    f: F,
    state: State<Fut, S::SinkItem>,
    _phantom: PhantomData<fn(U)>,
}

impl<S, U, Fut, F> With<S, U, Fut, F>
where S: Sink,
      F: FnMut(U) -> Fut,
      Fut: Future,
{
    unsafe_pinned!(sink -> S);
    unsafe_unpinned!(f -> F);
    unsafe_pinned!(state -> State<Fut, S::SinkItem>);
}

impl<S, U, Fut, F> Unpin for With<S, U, Fut, F>
where S: Sink + Unpin,
      F: FnMut(U) -> Fut,
      Fut: Future + Unpin,
{}

#[derive(Debug)]
enum State<Fut, T> {
    Empty,
    Process(Fut),
    Buffered(T),
}

impl<Fut, T> State<Fut, T> {
    fn as_pin_mut<'a>(
        self: PinMut<'a, Self>
    ) -> State<PinMut<'a, Fut>, PinMut<'a, T>> {
        unsafe {
            match PinMut::get_mut_unchecked(self) {
                State::Empty => State::Empty,
                State::Process(fut) => State::Process(PinMut::new_unchecked(fut)),
                State::Buffered(x) => State::Buffered(PinMut::new_unchecked(x)),
            }
        }
    }
}

pub fn new<S, U, Fut, F, E>(sink: S, f: F) -> With<S, U, Fut, F>
    where S: Sink,
          F: FnMut(U) -> Fut,
          Fut: Future<Output = Result<S::SinkItem, E>>,
          E: From<S::SinkError>,
{
    With {
        state: State::Empty,
        sink,
        f,
        _phantom: PhantomData,
    }
}

// Forwarding impl of Stream from the underlying sink
impl<S, U, Fut, F> Stream for With<S, U, Fut, F>
    where S: Stream + Sink,
          F: FnMut(U) -> Fut,
          Fut: Future
{
    type Item = S::Item;

    fn poll_next(mut self: PinMut<Self>, cx: &mut Context) -> Poll<Option<S::Item>> {
        self.sink().poll_next(cx)
    }
}

impl<S, U, Fut, F, E> With<S, U, Fut, F>
    where S: Sink,
          F: FnMut(U) -> Fut,
          Fut: Future<Output = Result<S::SinkItem, E>>,
          E: From<S::SinkError>,
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

    fn poll(self: &mut PinMut<Self>, cx: &mut Context) -> Poll<Result<(), E>> {
        let buffered = match self.state().as_pin_mut() {
            State::Empty => return Poll::Ready(Ok(())),
            State::Process(mut fut) => Some(try_ready!(fut.poll(cx))),
            State::Buffered(_) => None,
        };
        if let Some(buffered) = buffered {
            PinMut::set(self.state(), State::Buffered(buffered));
        }
        if let State::Buffered(item) = unsafe { mem::replace(PinMut::get_mut_unchecked(self.state()), State::Empty) } {
            Poll::Ready(self.sink().start_send(item).map_err(Into::into))
        } else {
            unreachable!()
        }
    }
}

impl<S, U, Fut, F, E> Sink for With<S, U, Fut, F>
    where S: Sink,
          F: FnMut(U) -> Fut,
          Fut: Future<Output = Result<S::SinkItem, E>>,
          E: From<S::SinkError>,
{
    type SinkItem = U;
    type SinkError = E;

    fn poll_ready(mut self: PinMut<Self>, cx: &mut Context) -> Poll<Result<(), Self::SinkError>> {
        self.poll(cx)
    }

    fn start_send(mut self: PinMut<Self>, item: Self::SinkItem) -> Result<(), Self::SinkError> {
        let item = (self.f())(item);
        PinMut::set(self.state(), State::Process(item));
        Ok(())
    }

    fn poll_flush(mut self: PinMut<Self>, cx: &mut Context) -> Poll<Result<(), Self::SinkError>> {
        try_ready!(self.poll(cx));
        try_ready!(self.sink().poll_flush(cx));
        Poll::Ready(Ok(()))
    }

    fn poll_close(mut self: PinMut<Self>, cx: &mut Context) -> Poll<Result<(), Self::SinkError>> {
        try_ready!(self.poll(cx));
        try_ready!(self.sink().poll_close(cx));
        Poll::Ready(Ok(()))
    }
}
