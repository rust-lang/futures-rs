use core::marker::Unpin;
use core::mem::PinMut;
use futures_core::future::Future;
use futures_core::stream::Stream;
use futures_core::task::{Poll, Context};

/// A stream combinator which skips elements of a stream while a predicate
/// holds.
///
/// This structure is produced by the `Stream::skip_while` method.
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct SkipWhile<S, R, P> where S: Stream {
    stream: S,
    pred: P,
    pending_fut: Option<R>,
    pending_item: Option<S::Item>,
    done_skipping: bool,
}

pub fn new<S, R, P>(s: S, p: P) -> SkipWhile<S, R, P>
    where S: Stream,
          P: FnMut(&S::Item) -> R,
          R: Future<Output = bool>,
{
    SkipWhile {
        stream: s,
        pred: p,
        pending_fut: None,
        pending_item: None,
        done_skipping: false,
    }
}

impl<S, R, P> SkipWhile<S, R, P> where S: Stream {
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
    unsafe_unpinned!(done_skipping -> bool);
}

impl<S: Unpin + Stream, R: Unpin, P> Unpin for SkipWhile<S, R, P> {}

/* TODO
// Forwarding impl of Sink from the underlying stream
impl<S, R, P> Sink for SkipWhile<S, R, P>
    where S: Sink + Stream, R: IntoFuture
{
    type SinkItem = S::SinkItem;
    type SinkError = S::SinkError;

    delegate_sink!(stream);
}
*/

impl<S, R, P> Stream for SkipWhile<S, R, P>
    where S: Stream,
          P: FnMut(&S::Item) -> R,
          R: Future<Output = bool>,
{
    type Item = S::Item;

    fn poll_next(mut self: PinMut<Self>, cx: &mut Context) -> Poll<Option<S::Item>> {
        if *self.done_skipping() {
            return self.stream().poll_next(cx);
        }

        loop {
            if self.pending_item().is_none() {
                let item = match ready!(self.stream().poll_next(cx)) {
                    Some(e) => e,
                    None => return Poll::Ready(None),
                };
                let fut = (self.pred())(&item);
                PinMut::set(self.pending_fut(), Some(fut));
                *self.pending_item() = Some(item);
            }

            let skipped = ready!(self.pending_fut().as_pin_mut().unwrap().poll(cx));
            let item = self.pending_item().take().unwrap();
            PinMut::set(self.pending_fut(), None);

            if !skipped {
                *self.done_skipping() = true;
                return Poll::Ready(Some(item))
            }
        }
    }
}
