use core::marker::Unpin;
use core::pin::Pin;
use futures_core::future::Future;
use futures_core::stream::{FusedStream, Stream};
use futures_core::task::{LocalWaker, Poll};
use pin_utils::{unsafe_pinned, unsafe_unpinned};

/// A stream combinator which skips elements of a stream while a predicate
/// holds.
///
/// This structure is produced by the `Stream::skip_while` method.
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct SkipWhile<St, Fut, F> where St: Stream {
    stream: St,
    f: F,
    pending_fut: Option<Fut>,
    pending_item: Option<St::Item>,
    done_skipping: bool,
}

impl<St: Unpin + Stream, Fut: Unpin, F> Unpin for SkipWhile<St, Fut, F> {}

impl<St, Fut, F> SkipWhile<St, Fut, F>
    where St: Stream,
          F: FnMut(&St::Item) -> Fut,
          Fut: Future<Output = bool>,
{
    unsafe_pinned!(stream: St);
    unsafe_unpinned!(f: F);
    unsafe_pinned!(pending_fut: Option<Fut>);
    unsafe_unpinned!(pending_item: Option<St::Item>);
    unsafe_unpinned!(done_skipping: bool);

    pub(super) fn new(stream: St, f: F) -> SkipWhile<St, Fut, F> {
        SkipWhile {
            stream,
            f,
            pending_fut: None,
            pending_item: None,
            done_skipping: false,
        }
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

impl<St: Stream + FusedStream, Fut, F> FusedStream for SkipWhile<St, Fut, F> {
    fn is_terminated(&self) -> bool {
        self.stream.is_terminated()
    }
}

impl<St, Fut, F> Stream for SkipWhile<St, Fut, F>
    where St: Stream,
          F: FnMut(&St::Item) -> Fut,
          Fut: Future<Output = bool>,
{
    type Item = St::Item;

    fn poll_next(
        mut self: Pin<&mut Self>,
        lw: &LocalWaker,
    ) -> Poll<Option<St::Item>> {
        if self.done_skipping {
            return self.as_mut().stream().poll_next(lw);
        }

        loop {
            if self.pending_item.is_none() {
                let item = match ready!(self.as_mut().stream().poll_next(lw)) {
                    Some(e) => e,
                    None => return Poll::Ready(None),
                };
                let fut = (self.as_mut().f())(&item);
                self.as_mut().pending_fut().set(Some(fut));
                *self.as_mut().pending_item() = Some(item);
            }

            let skipped = ready!(self.as_mut().pending_fut().as_pin_mut().unwrap().poll(lw));
            let item = self.as_mut().pending_item().take().unwrap();
            self.as_mut().pending_fut().set(None);

            if !skipped {
                *self.as_mut().done_skipping() = true;
                return Poll::Ready(Some(item))
            }
        }
    }
}

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
