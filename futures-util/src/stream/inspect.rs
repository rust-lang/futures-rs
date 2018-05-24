use core::mem::PinMut;
use core::marker::Unpin;

use futures_core::{Stream, Poll};
use futures_core::task;

/// Do something with the items of a stream, passing it on.
///
/// This is created by the `Stream::inspect` method.
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct Inspect<S, F> where S: Stream {
    stream: S,
    inspect: F,
}

pub fn new<S, F>(stream: S, f: F) -> Inspect<S, F>
    where S: Stream,
          F: FnMut(&S::Item) -> (),
{
    Inspect {
        stream: stream,
        inspect: f,
    }
}

impl<S: Stream, F> Inspect<S, F> {
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
    unsafe_unpinned!(inspect -> F);
}

unsafe impl<S: Stream + Unpin, F> Unpin for Inspect<S, F> {}

/* TODO
// Forwarding impl of Sink from the underlying stream
impl<S, F> Sink for Inspect<S, F>
    where S: Sink + Stream
{
    type SinkItem = S::SinkItem;
    type SinkError = S::SinkError;

    delegate_sink!(stream);
}
*/

impl<S, F> Stream for Inspect<S, F>
    where S: Stream,
          F: FnMut(&S::Item),
{
    type Item = S::Item;

    fn poll_next(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<Option<S::Item>> {
        let item = ready!(self.stream().poll_next(cx));
        Poll::Ready(item.map(|e| {
            (self.inspect())(&e);
            e
        }))
    }
}
