use core::fmt;
use core::marker::PhantomData;
use core::pin::Pin;
use futures_core::future::Future;
use futures_core::ready;
use futures_core::stream::{FusedStream, Stream};
use futures_core::task::{Context, Poll};
use futures_sink::Sink;
use pin_project_lite::pin_project;

pin_project! {
    /// Sink for the [`with`](super::SinkExt::with) method.
    #[must_use = "sinks do nothing unless polled"]
    pub struct With<Si, Item, U, Fut, F> {
        #[pin]
        sink: Si,
        f: F,
        #[pin]
        state: Option<Fut>,
        item: Option<Item>,
        _phantom: PhantomData<fn(U) -> Item>,
    }
}

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
            .field("item", &self.item)
            .finish()
    }
}

impl<Si, Item, U, Fut, F> With<Si, Item, U, Fut, F>
where
    Si: Sink<Item>,
    F: FnMut(U) -> Fut,
    Fut: Future,
{
    pub(super) fn new<E>(sink: Si, f: F) -> Self
    where
        Fut: Future<Output = Result<Item, E>>,
        E: From<Si::Error>,
    {
        Self { state: None, sink, f, item: None, _phantom: PhantomData }
    }
}

impl<Si, Item, U, Fut, F> Clone for With<Si, Item, U, Fut, F>
where
    Si: Clone,
    Item: Clone,
    F: Clone,
    Fut: Clone,
{
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
            sink: self.sink.clone(),
            f: self.f.clone(),
            item: self.item.clone(),
            _phantom: PhantomData,
        }
    }
}

// Forwarding impl of Stream from the underlying sink
impl<S, Item, U, Fut, F> Stream for With<S, Item, U, Fut, F>
where
    S: Stream + Sink<Item>,
    F: FnMut(U) -> Fut,
    Fut: Future,
{
    type Item = S::Item;

    delegate_stream!(sink);
}

impl<S, Item, U, Fut, F> FusedStream for With<S, Item, U, Fut, F>
where
    S: FusedStream + Sink<Item>,
    F: FnMut(U) -> Fut,
    Fut: Future,
{
    fn is_terminated(&self) -> bool {
        self.sink.is_terminated()
    }
}

impl<Si, Item, U, Fut, F, E> With<Si, Item, U, Fut, F>
where
    Si: Sink<Item>,
    F: FnMut(U) -> Fut,
    Fut: Future<Output = Result<Item, E>>,
    E: From<Si::Error>,
{
    delegate_access_inner!(sink, Si, ());

    /// Completes the processing of previous item if any.
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), E>> {
        let mut this = self.project();

        loop {
            if this.item.is_some() {
                // Check if the underlying sink is prepared for another item.
                // If it is, we have to send it without yielding in between.
                match this.sink.as_mut().poll_ready(cx)? {
                    Poll::Ready(()) => this.sink.start_send(this.item.take().unwrap())?,
                    Poll::Pending => match this.sink.as_mut().poll_flush(cx)? {
                        Poll::Ready(()) => continue, // check `poll_ready` again
                        Poll::Pending => return Poll::Pending,
                    },
                }
            }
            if let Some(fut) = this.state.as_mut().as_pin_mut() {
                let item = ready!(fut.poll(cx))?;
                *this.item = Some(item);
                this.state.set(None);
            }
            return Poll::Ready(Ok(()));
        }
    }
}

impl<Si, Item, U, Fut, F, E> Sink<U> for With<Si, Item, U, Fut, F>
where
    Si: Sink<Item>,
    F: FnMut(U) -> Fut,
    Fut: Future<Output = Result<Item, E>>,
    E: From<Si::Error>,
{
    type Error = E;

    fn poll_ready(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        ready!(self.as_mut().poll(cx))?;
        ready!(self.project().sink.poll_ready(cx)?);
        Poll::Ready(Ok(()))
    }

    fn start_send(self: Pin<&mut Self>, item: U) -> Result<(), Self::Error> {
        let mut this = self.project();

        assert!(this.state.is_none());
        this.state.set(Some((this.f)(item)));
        Ok(())
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        ready!(self.as_mut().poll(cx))?;
        ready!(self.project().sink.poll_flush(cx)?);
        Poll::Ready(Ok(()))
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        ready!(self.as_mut().poll(cx))?;
        ready!(self.project().sink.poll_close(cx)?);
        Poll::Ready(Ok(()))
    }
}
