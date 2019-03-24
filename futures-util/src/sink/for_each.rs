use core::marker::PhantomData;
use core::pin::Pin;
use futures_core::future::Future;
use futures_core::task::{Waker, Poll};
use futures_sink::Sink;
use pin_utils::{unsafe_pinned, unsafe_unpinned};

/// [`Sink`] for the [`for_each`] function.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct ForEach<F, Fut, T, E>
where
    F: FnMut(T) -> Fut,
    Fut: Future<Output = Result<(), E>>,
{
    f: F,
    future: Option<Fut>,
    _phantom: PhantomData<fn(T) -> E>,
}

/// Creates a [`Sink`] that will run a provided asynchronous closure on each item passed to it.
///
/// # Examples
///
/// ```
/// #![feature(async_await, await_macro, futures_api)]
/// # futures::executor::block_on(async {
/// use std::rc::Rc;
/// use std::cell::RefCell;
/// use futures::sink;
/// use futures::stream::{self, StreamExt};
///
/// let items = Rc::new(RefCell::new(vec![]));
///
/// let sink = sink::for_each(|i| {
///     let items = items.clone();
///     async move {
///         items.borrow_mut().push(i);
///         Ok::<(), ()>(())
///     }
/// });
///
/// await!(stream::iter(vec![1, 2, 3]).map(Ok).forward(sink));
///
/// assert_eq!(&*items.borrow(), &vec![1, 2, 3]);
/// # });
/// ```
pub fn for_each<F, Fut, T, E>(f: F) -> ForEach<F, Fut, T, E>
where
    F: FnMut(T) -> Fut,
    Fut: Future<Output = Result<(), E>>,
{
    ForEach {
        f,
        future: None,
        _phantom: PhantomData,
    }
}

impl<F, Fut, T, E> ForEach<F, Fut, T, E>
where
    F: FnMut(T) -> Fut,
    Fut: Future<Output = Result<(), E>>,
{
    unsafe_unpinned!(f: F);
    unsafe_pinned!(future: Option<Fut>);
}

impl<F, Fut, T, E> Unpin for ForEach<F, Fut, T, E>
where
    F: FnMut(T) -> Fut,
    Fut: Future<Output = Result<(), E>> + Unpin,
{}

impl<F, Fut, T, E> Sink<T> for ForEach<F, Fut, T, E>
where
    F: FnMut(T) -> Fut,
    Fut: Future<Output = Result<(), E>>,
{
    type SinkError = E;

    fn poll_ready(
        self: Pin<&mut Self>,
        waker: &Waker
    ) -> Poll<Result<(), Self::SinkError>>
    {
        self.poll_flush(waker)
    }

    fn start_send(
        mut self: Pin<&mut Self>,
        item: T
    ) -> Result<(), Self::SinkError>
    {
        debug_assert!(self.as_mut().future().is_none());
        let future = (self.as_mut().f())(item);
        self.future().set(Some(future));
        Ok(())
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        waker: &Waker
    ) -> Poll<Result<(), Self::SinkError>>
    {
        if let Some(future) = self.as_mut().future().as_pin_mut() {
            ready!(future.poll(waker))?;
            self.future().set(None);
        }
        Poll::Ready(Ok(()))
    }

    fn poll_close(
        self: Pin<&mut Self>,
        waker: &Waker
    ) -> Poll<Result<(), Self::SinkError>>
    {
        self.poll_flush(waker)
    }
}
