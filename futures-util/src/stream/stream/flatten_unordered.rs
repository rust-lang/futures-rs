use alloc::sync::Arc;
use core::{
    cell::UnsafeCell,
    fmt,
    num::NonZeroUsize,
    pin::Pin,
    sync::atomic::{AtomicU8, Ordering},
};

use pin_project_lite::pin_project;

use futures_core::{
    future::Future,
    ready,
    stream::{FusedStream, Stream},
    task::{Context, Poll, Waker},
};
#[cfg(feature = "sink")]
use futures_sink::Sink;
use futures_task::{waker, ArcWake};

use crate::stream::FuturesUnordered;

/// There is nothing to poll and stream isn't being
/// polled or waking at the moment.
const NONE: u8 = 0;

/// Inner streams need to be polled.
const NEED_TO_POLL_INNER_STREAMS: u8 = 1;

/// The base stream needs to be polled.
const NEED_TO_POLL_STREAM: u8 = 0b10;

/// It needs to poll base stream and inner streams.
const NEED_TO_POLL_ALL: u8 = NEED_TO_POLL_INNER_STREAMS | NEED_TO_POLL_STREAM;

/// The current stream is being polled at the moment.
const POLLING: u8 = 0b100;

/// Inner streams are being woken at the moment.
const WAKING_INNER_STREAMS: u8 = 0b1000;

/// The base stream is being woken at the moment.
const WAKING_STREAM: u8 = 0b10000;

/// The base stream or inner streams are being woken at the moment.
const WAKING_ALL: u8 = WAKING_STREAM | WAKING_INNER_STREAMS;

/// Determines what needs to be polled, and is stream being polled at the
/// moment or not.
#[derive(Clone, Debug)]
struct SharedPollState {
    state: Arc<AtomicU8>,
}

impl SharedPollState {
    /// Constructs new `SharedPollState` with the given state.
    fn new(value: u8) -> SharedPollState {
        SharedPollState { state: Arc::new(AtomicU8::new(value)) }
    }

    /// Attempts to start polling, returning stored state in case of success.
    /// Returns `None` if some waker is waking at the moment.
    fn start_polling(&self) -> Option<(u8, PollStateBomb<'_, impl FnOnce(&SharedPollState)>)> {
        self.state
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |value| {
                if value & WAKING_ALL == NONE {
                    Some(POLLING)
                } else {
                    None
                }
            })
            .ok()
            .map(|value| {
                (
                    value,
                    PollStateBomb::new(self, |state| {
                        state.stop_polling(NEED_TO_POLL_ALL);
                    }),
                )
            })
    }

    /// Starts the waking process and performs bitwise or with the given value.
    fn start_waking(
        &self,
        to_poll: u8,
        waking: u8,
    ) -> (u8, PollStateBomb<'_, impl FnOnce(&SharedPollState)>) {
        let value = self.state.fetch_or(to_poll | waking, Ordering::SeqCst);

        (
            value,
            PollStateBomb::new(self, move |state| {
                state.stop_waking(waking);
            }),
        )
    }

    /// Toggles state to non-waking, allowing to start polling.
    fn stop_waking(&self, waking: u8) -> u8 {
        self.state.fetch_and(!waking, Ordering::SeqCst)
    }

    /// Sets current state to `!POLLING`, allowing to use wakers.
    fn stop_polling(&self, to_poll: u8) -> u8 {
        self.state
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |value| {
                Some((value | to_poll) & !POLLING)
            })
            .unwrap()
    }
}

/// Used to execute some function on the given state when dropped.
struct PollStateBomb<'a, F: FnOnce(&SharedPollState)> {
    state: &'a SharedPollState,
    drop: Option<F>,
}

impl<'a, F: FnOnce(&SharedPollState)> PollStateBomb<'a, F> {
    /// Constructs new bomb with the given state.
    fn new(state: &'a SharedPollState, drop: F) -> Self {
        Self { state, drop: Some(drop) }
    }

    /// Deactivates bomb, forces it to not call provided function when dropped.
    fn deactivate(mut self) {
        self.drop.take();
    }
}

impl<F: FnOnce(&SharedPollState)> Drop for PollStateBomb<'_, F> {
    fn drop(&mut self) {
        if let Some(drop) = self.drop.take() {
            (drop)(self.state);
        }
    }
}

/// Will update state with the provided value on `wake_by_ref` call
/// and then, if there is a need, call `inner_waker`.
struct InnerWaker {
    inner_waker: UnsafeCell<Option<Waker>>,
    poll_state: SharedPollState,
    need_to_poll: u8,
}

unsafe impl Send for InnerWaker {}
unsafe impl Sync for InnerWaker {}

impl InnerWaker {
    /// Replaces given waker's inner_waker for polling stream/futures which will
    /// update poll state on `wake_by_ref` call. Use only if you need several
    /// contexts.
    ///
    /// ## Safety
    ///
    /// This function will modify waker's `inner_waker` via `UnsafeCell`, so
    /// it should be used only during `POLLING` phase.
    unsafe fn replace_waker(self_arc: &mut Arc<Self>, cx: &Context<'_>) -> Waker {
        *self_arc.inner_waker.get() = cx.waker().clone().into();
        waker(self_arc.clone())
    }

    // Flags state that waking is started for the waker with the given value.
    fn start_waking(&self) -> (u8, PollStateBomb<'_, impl FnOnce(&SharedPollState)>) {
        self.poll_state.start_waking(self.need_to_poll, self.need_to_poll << 3)
    }
}

impl ArcWake for InnerWaker {
    fn wake_by_ref(self_arc: &Arc<Self>) {
        let (poll_state_value, state_bomb) = self_arc.start_waking();

        // Only call waker if stream isn't being polled because of safety reasons.
        // Waker will be called at the end of polling if state was changed.
        if poll_state_value & POLLING == NONE {
            if let Some(inner_waker) =
                unsafe { self_arc.inner_waker.get().as_ref().cloned().flatten() }
            {
                // First, stop waking to allow polling stream
                drop(state_bomb);
                // Wake up inner waker
                inner_waker.wake();
            }
        }
    }
}

pin_project! {
    /// Future which contains optional stream.
    ///
    /// If it's `Some`, it will attempt to call `poll_next` on it,
    /// returning `Some((item, next_item_fut))` in case of `Poll::Ready(Some(...))`
    /// or `None` in case of `Poll::Ready(None)`.
    ///
    /// If `poll_next` will return `Poll::Pending`, it will be forwarded to
    /// the future and current task will be notified by waker.
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    struct PollStreamFut<St> {
        #[pin]
        stream: Option<St>,
    }
}

impl<St> PollStreamFut<St> {
    /// Constructs new `PollStreamFut` using given `stream`.
    fn new(stream: impl Into<Option<St>>) -> Self {
        Self { stream: stream.into() }
    }
}

impl<St: Stream + Unpin> Future for PollStreamFut<St> {
    type Output = Option<(St::Item, PollStreamFut<St>)>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut stream = self.project().stream;

        let item = if let Some(stream) = stream.as_mut().as_pin_mut() {
            ready!(stream.poll_next(cx))
        } else {
            None
        };
        let next_item_fut = PollStreamFut::new(stream.get_mut().take());
        let out = item.map(|item| (item, next_item_fut));

        Poll::Ready(out)
    }
}

pin_project! {
    /// Stream for the [`flatten_unordered`](super::StreamExt::flatten_unordered)
    /// method.
    #[project = FlattenUnorderedProj]
    #[must_use = "streams do nothing unless polled"]
    pub struct FlattenUnordered<St> where St: Stream {
        #[pin]
        inner_streams: FuturesUnordered<PollStreamFut<St::Item>>,
        #[pin]
        stream: St,
        poll_state: SharedPollState,
        limit: Option<NonZeroUsize>,
        is_stream_done: bool,
        inner_streams_waker: Arc<InnerWaker>,
        stream_waker: Arc<InnerWaker>,
    }
}

impl<St> fmt::Debug for FlattenUnordered<St>
where
    St: Stream + fmt::Debug,
    St::Item: Stream + fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FlattenUnordered")
            .field("poll_state", &self.poll_state)
            .field("inner_streams", &self.inner_streams)
            .field("limit", &self.limit)
            .field("stream", &self.stream)
            .field("is_stream_done", &self.is_stream_done)
            .finish()
    }
}

impl<St> FlattenUnordered<St>
where
    St: Stream,
    St::Item: Stream + Unpin,
{
    pub(super) fn new(stream: St, limit: Option<usize>) -> FlattenUnordered<St> {
        let poll_state = SharedPollState::new(NEED_TO_POLL_STREAM);

        FlattenUnordered {
            inner_streams: FuturesUnordered::new(),
            stream,
            is_stream_done: false,
            limit: limit.and_then(NonZeroUsize::new),
            inner_streams_waker: Arc::new(InnerWaker {
                inner_waker: UnsafeCell::new(None),
                poll_state: poll_state.clone(),
                need_to_poll: NEED_TO_POLL_INNER_STREAMS,
            }),
            stream_waker: Arc::new(InnerWaker {
                inner_waker: UnsafeCell::new(None),
                poll_state: poll_state.clone(),
                need_to_poll: NEED_TO_POLL_STREAM,
            }),
            poll_state,
        }
    }

    delegate_access_inner!(stream, St, ());
}

impl<St> FlattenUnorderedProj<'_, St>
where
    St: Stream,
{
    /// Checks if current `inner_streams` size is less than optional limit.
    fn is_exceeded_limit(&self) -> bool {
        self.limit.map_or(false, |limit| self.inner_streams.len() >= limit.get())
    }
}

impl<St> FusedStream for FlattenUnordered<St>
where
    St: FusedStream,
    St::Item: FusedStream + Unpin,
{
    fn is_terminated(&self) -> bool {
        self.stream.is_terminated() && self.inner_streams.is_empty()
    }
}

impl<St> Stream for FlattenUnordered<St>
where
    St: Stream,
    St::Item: Stream + Unpin,
{
    type Item = <St::Item as Stream>::Item;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut next_item = None;
        let mut need_to_poll_next = NONE;

        let mut this = self.as_mut().project();

        let (mut poll_state_value, state_bomb) = match this.poll_state.start_polling() {
            Some(value) => value,
            _ => {
                // Waker was called, just wait for the next poll
                return Poll::Pending;
            }
        };

        if poll_state_value & NEED_TO_POLL_STREAM != NONE {
            // Safety: now state is `POLLING`.
            let stream_waker = unsafe { InnerWaker::replace_waker(this.stream_waker, cx) };

            // Here we need to poll the base stream.
            //
            // To improve performance, we will attempt to place as many items as we can
            // to the `FuturesUnordered` bucket before polling inner streams
            loop {
                if this.is_exceeded_limit() || *this.is_stream_done {
                    // We either exceeded the limit or the stream is exhausted
                    if !*this.is_stream_done {
                        // The stream needs to be polled in the next iteration
                        need_to_poll_next |= NEED_TO_POLL_STREAM;
                    }

                    break;
                } else {
                    match this.stream.as_mut().poll_next(&mut Context::from_waker(&stream_waker)) {
                        Poll::Ready(Some(inner_stream)) => {
                            // Add new stream to the inner streams bucket
                            this.inner_streams.as_mut().push(PollStreamFut::new(inner_stream));
                            // Inner streams must be polled afterward
                            poll_state_value |= NEED_TO_POLL_INNER_STREAMS;
                        }
                        Poll::Ready(None) => {
                            // Mark the stream as done
                            *this.is_stream_done = true;
                        }
                        Poll::Pending => {
                            break;
                        }
                    }
                }
            }
        }

        if poll_state_value & NEED_TO_POLL_INNER_STREAMS != NONE {
            // Safety: now state is `POLLING`.
            let inner_streams_waker =
                unsafe { InnerWaker::replace_waker(this.inner_streams_waker, cx) };

            match this
                .inner_streams
                .as_mut()
                .poll_next(&mut Context::from_waker(&inner_streams_waker))
            {
                Poll::Ready(Some(Some((item, next_item_fut)))) => {
                    // Push next inner stream item future to the list of inner streams futures
                    this.inner_streams.as_mut().push(next_item_fut);
                    // Take the received item
                    next_item = Some(item);
                    // On the next iteration, inner streams must be polled again
                    need_to_poll_next |= NEED_TO_POLL_INNER_STREAMS;
                }
                Poll::Ready(Some(None)) => {
                    // On the next iteration, inner streams must be polled again
                    need_to_poll_next |= NEED_TO_POLL_INNER_STREAMS;
                }
                _ => {}
            }
        }

        // We didn't have any `poll_next` panic, so it's time to deactivate the bomb
        state_bomb.deactivate();
        // Stop polling and swap the latest state
        poll_state_value = this.poll_state.stop_polling(need_to_poll_next);
        let is_done = *this.is_stream_done && this.inner_streams.is_empty();

        if next_item.is_some() || is_done {
            Poll::Ready(next_item)
        } else {
            // We need to call the waker if state was changed during the polling phase
            if poll_state_value & NEED_TO_POLL_ALL != NONE
                // or we need to poll the stream and didn't reach the limit yet
                || need_to_poll_next & NEED_TO_POLL_STREAM != NONE && !this.is_exceeded_limit()
                // or we need to poll inner streams again
                || need_to_poll_next & NEED_TO_POLL_INNER_STREAMS != NONE
            {
                cx.waker().wake_by_ref();
            }

            Poll::Pending
        }
    }
}

// Forwarding impl of Sink from the underlying stream
#[cfg(feature = "sink")]
impl<St, Item> Sink<Item> for FlattenUnordered<St>
where
    St: Stream + Sink<Item>,
    St::Item: Stream,
{
    type Error = St::Error;

    delegate_sink!(stream, Item);
}
