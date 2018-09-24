use core::marker::Unpin;
use core::pin::Pin;
use futures_core::stream::Stream;
use futures_core::task::{self, Poll};
use pin_utils::unsafe_pinned;

/// A combinator used to flatten a stream-of-streams into one long stream of
/// elements.
///
/// This combinator is created by the `Stream::flatten` method.
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct Flatten<St>
    where St: Stream,
{
    stream: St,
    next: Option<St::Item>,
}

impl<St: Stream> Unpin for Flatten<St>
where St: Stream + Unpin,
      St::Item: Stream + Unpin,
{}

impl<St: Stream> Flatten<St>
where St: Stream,
      St::Item: Stream,
{
    unsafe_pinned!(stream: St);
    unsafe_pinned!(next: Option<St::Item>);

    pub(super) fn new(stream: St) -> Flatten<St>{
        Flatten { stream, next: None, }
    }

    /// Acquires a reference to the underlying stream that this combinator is
    /// pulling from.
    pub fn get_ref(&self) -> &St {
        &self.stream
    }

    /// Acquires a mutable reference to the underlying stream that this
    /// combinator is pulling from.
    ///
    /// Note that care must be taken to avoid tampering with the state of the
    /// stream which may otherwise confuse this combinator.
    pub fn get_mut(&mut self) -> &mut St {
        &mut self.stream
    }

    /// Consumes this combinator, returning the underlying stream.
    ///
    /// Note that this may discard intermediate state of this combinator, so
    /// care should be taken to avoid losing resources when this is called.
    pub fn into_inner(self) -> St {
        self.stream
    }
}

impl<St> Stream for Flatten<St>
    where St: Stream,
          St::Item: Stream,
{
    type Item = <St::Item as Stream>::Item;

    fn poll_next(
        mut self: Pin<&mut Self>,
        lw: &LocalWaker,
    ) -> Poll<Option<Self::Item>> {
        loop {
            if self.next().as_pin_mut().is_none() {
                match ready!(self.stream().poll_next(cx)) {
                    Some(e) => Pin::set(self.next(), Some(e)),
                    None => return Poll::Ready(None),
                }
            }
            let item = ready!(self.next().as_pin_mut().unwrap().poll_next(cx));
            if item.is_some() {
                return Poll::Ready(item);
            } else {
                Pin::set(self.next(), None);
            }
        }
    }
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
