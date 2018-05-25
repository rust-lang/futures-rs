use core::mem::PinMut;



use futures_core::{Poll, Stream};
use futures_core::task;

/// A combinator used to flatten a stream-of-streams into one long stream of
/// elements.
///
/// This combinator is created by the `Stream::flatten` method.
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct Flatten<S>
    where S: Stream,
{
    stream: S,
    next: Option<S::Item>,
}

pub fn new<S>(s: S) -> Flatten<S>
    where S: Stream,
          S::Item: Stream,
{
    Flatten {
        stream: s,
        next: None,
    }
}

impl<S: Stream> Flatten<S> {
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
    unsafe_pinned!(next -> Option<S::Item>);
}

/* TODO
// Forwarding impl of Sink from the underlying stream
impl<S> Sink for Flatten<S>
    where S: Sink + Stream
{
    type SinkItem = S::SinkItem;
    type SinkError = S::SinkError;

    delegate_sink!(stream);
}
 */

impl<S> Stream for Flatten<S>
    where S: Stream,
          S::Item: Stream,
{
    type Item = <S::Item as Stream>::Item;

    fn poll_next(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<Option<Self::Item>> {
        loop {
            if self.next().as_pin_mut().is_none() {
                match ready!(self.stream().poll_next(cx)) {
                    Some(e) => PinMut::set(self.next(), Some(e)),
                    None => return Poll::Ready(None),
                }
            }
            let item = ready!(self.next().as_pin_mut().unwrap().poll_next(cx));
            if item.is_some() {
                return Poll::Ready(item);
            } else {
                PinMut::set(self.next(), None);
            }
        }
    }
}
