use core::mem::PinMut;

use {PinMutExt, OptionExt};

use futures_core::{Future, Poll, Stream};
use futures_core::task;

/// A stream combinator used to filter the results of a stream and only yield
/// some values.
///
/// This structure is produced by the `Stream::filter` method.
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct Filter<S, R, P>
    where S: Stream,
          P: FnMut(&S::Item) -> R,
          R: Future<Output = bool>,
{
    stream: S,
    pred: P,
    pending_fut: Option<R>,
    pending_item: Option<S::Item>,
}

pub fn new<S, R, P>(s: S, pred: P) -> Filter<S, R, P>
    where S: Stream,
          P: FnMut(&S::Item) -> R,
          R: Future<Output = bool>,
{
    Filter {
        stream: s,
        pred: pred,
        pending_fut: None,
        pending_item: None,
    }
}

impl<S, R, P> Filter<S, R, P>
    where S: Stream,
          P: FnMut(&S::Item) -> R,
          R: Future<Output = bool>,
{
    /// Acquires a reference to the underlying stream that this combinator is
    /// pulling from.
    pub fn get_ref(&self) -> &S {
        &self.stream
    }

    /// Acquires a mutable reference to the underlying stream that this
    /// combinator is pulling from.
    ///
    /// Note that care must be taken to avoid tampering with the state of the
    /// stream which may otherwise confuse this combinator.
    pub fn get_mut(&mut self) -> &mut S {
        &mut self.stream
    }

    /// Consumes this combinator, returning the underlying stream.
    ///
    /// Note that this may discard intermediate state of this combinator, so
    /// care should be taken to avoid losing resources when this is called.
    pub fn into_inner(self) -> S {
        self.stream
    }

    unsafe_pinned!(stream -> S);
    unsafe_unpinned!(pred -> P);
    unsafe_pinned!(pending_fut -> Option<R>);
    unsafe_unpinned!(pending_item -> Option<S::Item>);
}

/* TODO
// Forwarding impl of Sink from the underlying stream
impl<S, R, P> Sink for Filter<S, R, P>
    where S: Stream,
          P: FnMut(&S::Item) -> R,
          R: Future<Item = bool>,
          S: Sink,
{
    type SinkItem = S::SinkItem;
    type SinkError = S::SinkError;

    delegate_sink!(stream);
}
*/

impl<S, R, P> Stream for Filter<S, R, P>
    where S: Stream,
          P: FnMut(&S::Item) -> R,
          R: Future<Output = bool>,
{
    type Item = S::Item;

    fn poll_next(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<Option<S::Item>> {
        loop {
            if self.pending_fut().as_pin_mut().is_none() {
                let item = match try_ready!(self.stream().poll_next(cx)) {
                    Some(e) => e,
                    None => return Poll::Ready(None),
                };
                let fut = (self.pred())(&item);
                self.pending_fut().assign(Some(fut));
                *self.pending_item() = Some(item);
            }

            let yield_item = try_ready!(self.pending_fut().as_pin_mut().unwrap().poll(cx));
            self.pending_fut().assign(None);
            let item = self.pending_item().take().unwrap();

            if yield_item {
                return Poll::Ready(Some(item));
            }
        }
    }
}
