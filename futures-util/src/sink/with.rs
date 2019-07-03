use core::fmt;
use core::marker::PhantomData;
use core::mem;
use core::pin::Pin;
use futures_core::future::Future;
use futures_core::stream::Stream;
use futures_core::task::{Context, Poll};
use futures_sink::Sink;
use pin_utils::{unsafe_pinned, unsafe_unpinned};

/// Sink for the [`with`](super::SinkExt::with) method.
#[must_use = "sinks do nothing unless polled"]
pub struct With<Si, Item, U, Fut, F> {
    sink: Si,
    f: F,
    state: State<Fut, Item>,
    _phantom: PhantomData<fn(U)>,
}

impl<Si, Item, U, Fut, F> Unpin for With<Si, Item, U, Fut, F>
where
    Si: Unpin,
    Fut: Unpin,
{}

impl<Si, Item, U, Fut, F> fmt::Debug for With<Si, Item, U, Fut, F>
where
    Si: fmt::Debug,
    Fut: fmt::Debug,
    Item: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("With")
            .field("sink", &self.sink)
            .field("state", &self.state)
            .finish()
    }
}

impl<Si, Item, U, Fut, F> With<Si, Item, U, Fut, F>
where Si: Sink<Item>,
      F: FnMut(U) -> Fut,
      Fut: Future,
{
    unsafe_pinned!(sink: Si);
    unsafe_unpinned!(f: F);
    unsafe_pinned!(state: State<Fut, Item>);

    pub(super) fn new<E>(sink: Si, f: F) -> Self
        where
            Fut: Future<Output = Result<Item, E>>,
            E: From<Si::Error>,
    {
        With {
            state: State::Empty,
            sink,
            f,
            _phantom: PhantomData,
        }
    }
}

#[derive(Debug)]
enum State<Fut, T> {
    Empty,
    Process(Fut),
    Buffered(T),
}

impl<Fut, T> State<Fut, T> {
    #[allow(clippy::wrong_self_convention)]
    fn as_pin_mut<'a>(
        self: Pin<&'a mut Self>,
    ) -> State<Pin<&'a mut Fut>, Pin<&'a mut T>> {
        unsafe {
            match self.get_unchecked_mut() {
                State::Empty =>
                    State::Empty,
                State::Process(fut) =>
                    State::Process(Pin::new_unchecked(fut)),
                State::Buffered(item) =>
                    State::Buffered(Pin::new_unchecked(item)),
            }
        }
    }
}

// Forwarding impl of Stream from the underlying sink
impl<S, Item, U, Fut, F> Stream for With<S, Item, U, Fut, F>
    where S: Stream + Sink<Item>,
          F: FnMut(U) -> Fut,
          Fut: Future
{
    type Item = S::Item;

    fn poll_next(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<S::Item>> {
        self.sink().poll_next(cx)
    }
}

impl<Si, Item, U, Fut, F, E> With<Si, Item, U, Fut, F>
    where Si: Sink<Item>,
          F: FnMut(U) -> Fut,
          Fut: Future<Output = Result<Item, E>>,
          E: From<Si::Error>,
{
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

    fn poll(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), E>> {
        let buffered = match self.as_mut().state().as_pin_mut() {
            State::Empty => return Poll::Ready(Ok(())),
            State::Process(fut) => Some(ready!(fut.poll(cx))?),
            State::Buffered(_) => None,
        };
        if let Some(buffered) = buffered {
            self.as_mut().state().set(State::Buffered(buffered));
        }
        if let State::Buffered(item) = unsafe { mem::replace(self.as_mut().state().get_unchecked_mut(), State::Empty) } {
            Poll::Ready(self.as_mut().sink().start_send(item).map_err(Into::into))
        } else {
            unreachable!()
        }
    }
}

impl<Si, Item, U, Fut, F, E> Sink<U> for With<Si, Item, U, Fut, F>
    where Si: Sink<Item>,
          F: FnMut(U) -> Fut,
          Fut: Future<Output = Result<Item, E>>,
          E: From<Si::Error>,
{
    type Error = E;

    fn poll_ready(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        self.poll(cx)
    }

    fn start_send(
        mut self: Pin<&mut Self>,
        item: U,
    ) -> Result<(), Self::Error> {
        let item = (self.as_mut().f())(item);
        self.as_mut().state().set(State::Process(item));
        Ok(())
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        ready!(self.as_mut().poll(cx))?;
        ready!(self.as_mut().sink().poll_flush(cx))?;
        Poll::Ready(Ok(()))
    }

    fn poll_close(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        ready!(self.as_mut().poll(cx))?;
        ready!(self.as_mut().sink().poll_close(cx))?;
        Poll::Ready(Ok(()))
    }
}
