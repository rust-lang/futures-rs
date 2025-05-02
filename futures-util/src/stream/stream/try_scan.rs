use crate::unfold_state::UnfoldState;
use core::fmt;
use core::pin::Pin;
use futures_core::stream::{FusedStream, Stream};
use futures_core::task::{Context, Poll};
use futures_core::{ready, TryFuture};
#[cfg(feature = "sink")]
use futures_sink::Sink;
use pin_project_lite::pin_project;

pin_project! {
    /// TryStream for the [`try_scan`](super::StreamExt::try_scan) method.
    #[must_use = "streams do nothing unless polled"]
    pub struct TryScan<St, S, Fut, F> {
        #[pin]
        stream: St,
        f: F,
        #[pin]
        state: UnfoldState<S, Fut>,
    }
}

impl<St, S, Fut, F> fmt::Debug for TryScan<St, S, Fut, F>
where
    St: Stream + fmt::Debug,
    St::Item: fmt::Debug,
    S: fmt::Debug,
    Fut: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TryScan")
            .field("stream", &self.stream)
            .field("state", &self.state)
            .field("done_taking", &self.is_done_taking())
            .finish()
    }
}

impl<St: Stream, S, Fut, F> TryScan<St, S, Fut, F> {
    /// Checks if internal state is `None`.
    fn is_done_taking(&self) -> bool {
        matches!(self.state, UnfoldState::Empty)
    }
}

impl<B, St, S, Fut, F> TryScan<St, S, Fut, F>
where
    St: Stream,
    F: FnMut(S, St::Item) -> Fut,
    Fut: TryFuture<Ok = Option<(S, B)>>,
{
    pub(super) fn new(stream: St, initial_state: S, f: F) -> Self {
        Self { stream, f, state: UnfoldState::Value { value: initial_state } }
    }
}

impl<B, St, S, Fut, F> Stream for TryScan<St, S, Fut, F>
where
    St: Stream,
    F: FnMut(S, St::Item) -> Fut,
    Fut: TryFuture<Ok = Option<(S, B)>>,
{
    type Item = Result<B, Fut::Error>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.is_done_taking() {
            return Poll::Ready(None);
        }

        let mut this = self.project();

        Poll::Ready(loop {
            if let Some(fut) = this.state.as_mut().project_future() {
                match ready!(fut.try_poll(cx)) {
                    Ok(None) => {
                        this.state.set(UnfoldState::Empty);
                        break None;
                    }
                    Ok(Some((state, item))) => {
                        this.state.set(UnfoldState::Value { value: state });
                        break Some(Ok(item));
                    }
                    Err(e) => {
                        this.state.set(UnfoldState::Empty);
                        break Some(Err(e));
                    }
                }
            } else if let Some(item) = ready!(this.stream.as_mut().poll_next(cx)) {
                let state = this.state.as_mut().take_value().unwrap();
                this.state.set(UnfoldState::Future { future: (this.f)(state, item) })
            } else {
                break None;
            }
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        if self.is_done_taking() {
            (0, Some(0))
        } else {
            self.stream.size_hint() // can't know a lower bound, due to the predicate
        }
    }
}

impl<B, St, S, Fut, F> FusedStream for TryScan<St, S, Fut, F>
where
    St: FusedStream,
    F: FnMut(S, St::Item) -> Fut,
    Fut: TryFuture<Ok = Option<(S, B)>>,
{
    fn is_terminated(&self) -> bool {
        self.is_done_taking()
            || !matches!(self.state, UnfoldState::Future { .. }) && self.stream.is_terminated()
    }
}

// Forwarding impl of Sink from the underlying stream
#[cfg(feature = "sink")]
impl<St, S, Fut, F, Item> Sink<Item> for TryScan<St, S, Fut, F>
where
    St: Stream + Sink<Item>,
{
    type Error = St::Error;

    delegate_sink!(stream, Item);
}
