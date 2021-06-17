use futures_core::task::{Context, Poll};
use futures_core::Stream;
use std::pin::Pin;

/// Stream for the [`poll_immediate`](poll_immediate()) function.
#[derive(Debug, Clone)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct PollImmediate<S>(Option<S>);

impl<T, S> Stream for PollImmediate<S>
where
  S: Stream<Item = T>,
{
    type Item = Poll<T>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        unsafe {
            // # Safety
            // We never move the inner value until it is done. We only get a reference to it.
            let inner = &mut self.get_unchecked_mut().0;
            let fut = match inner.as_mut() {
                // inner is gone, so we can continue to signal that the stream is closed.
                None => return Poll::Ready(None),
                Some(inner) => inner,
            };
            let stream = Pin::new_unchecked(fut);
            match stream.poll_next(cx) {
                Poll::Ready(Some(t)) => Poll::Ready(Some(Poll::Ready(t))),
                Poll::Ready(None) => {
                    // # Safety
                    // The inner stream is done, so we need to drop it. We do it without moving it
                    // by using drop in place. We then write over the value without trying to drop it first
                    // This should uphold all the safety requirements of `Pin`
                    std::ptr::drop_in_place(inner);
                    std::ptr::write(inner, None);
                    Poll::Ready(None)
                }
                Poll::Pending => Poll::Ready(Some(Poll::Pending)),
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.as_ref().map_or((0, Some(0)), Stream::size_hint)
    }
}

impl<S: Stream> super::FusedStream for PollImmediate<S> {
    fn is_terminated(&self) -> bool {
        self.0.is_none()
    }
}

/// Creates a new stream that never blocks when awaiting it. This is useful
/// when immediacy is more important than waiting for the next item to be ready
///
/// # Examples
///
/// ```
/// # futures::executor::block_on(async {
/// use futures::stream::{self, StreamExt};
/// use futures::task::Poll;
///
/// let mut r = stream::poll_immediate(Box::pin(stream::iter(1_u32..3)));
/// assert_eq!(r.next().await, Some(Poll::Ready(1)));
/// assert_eq!(r.next().await, Some(Poll::Ready(2)));
/// assert_eq!(r.next().await, None);
///
/// let mut p = stream::poll_immediate(Box::pin(stream::once(async {
///     futures::pending!();
///     42_u8
/// })));
/// assert_eq!(p.next().await, Some(Poll::Pending));
/// assert_eq!(p.next().await, Some(Poll::Ready(42)));
/// assert_eq!(p.next().await, None);
/// # });
/// ```
pub fn poll_immediate<S: Stream>(s: S) -> PollImmediate<S> {
    super::assert_stream::<Poll<S::Item>, PollImmediate<S>>(PollImmediate(Some(s)))
}
