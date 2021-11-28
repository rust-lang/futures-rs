use crate::stream::FuturesUnordered;
use alloc::sync::Arc;
use core::cell::UnsafeCell;
use core::fmt;
use core::num::NonZeroUsize;
use core::pin::Pin;
use core::sync::atomic::{AtomicU8, Ordering};
use futures_core::future::Future;
use futures_core::stream::FusedStream;
use futures_core::stream::Stream;
use futures_core::{
    ready,
    task::{Context, Poll, Waker},
};
#[cfg(feature = "sink")]
use futures_sink::Sink;
use futures_task::{waker, ArcWake};
use pin_project_lite::pin_project;

/// Indicates that there is nothing to poll and stream isn't being polled at
/// the moment.
const NONE: u8 = 0;

/// This indicates that inner streams need to be polled.
const NEED_TO_POLL_INNER_STREAMS: u8 = 1;

/// This indicates that stream needs to be polled.
const NEED_TO_POLL_STREAM: u8 = 0b10;

/// This indicates that it needs to poll stream and inner streams.
const NEED_TO_POLL_ALL: u8 = NEED_TO_POLL_INNER_STREAMS | NEED_TO_POLL_STREAM;

/// This indicates that the current stream is being polled at the moment.
const POLLING: u8 = 0b100;

/// This indicates that inner streams are being waked at the moment.
const WAKING_INNER_STREAMS: u8 = 0b1000;

/// This indicates that the current stream is being waked at the moment.
const WAKING_STREAM: u8 = 0b10000;

/// This indicates that the current stream or inner streams are being waked at the moment.
const WAKING_ANYTHING: u8 = WAKING_STREAM | WAKING_INNER_STREAMS;

/// Determines what needs to be polled, and is stream being polled at the
/// moment or not.
#[derive(Clone, Debug)]
struct SharedPollState {
    state: Arc<AtomicU8>,
}

impl SharedPollState {
    /// Constructs new `SharedPollState` with given state.
    fn new(state: u8) -> SharedPollState {
        SharedPollState { state: Arc::new(AtomicU8::new(state)) }
    }

    /// Attempts to start polling, returning stored state in case of success.
    /// Returns `None` if state some waker is waking at the moment.
    fn start_polling(&self) -> Option<u8> {
        self.state
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |state| {
                if state & WAKING_ANYTHING == NONE {
                    Some(POLLING)
                } else {
                    None
                }
            })
            .ok()
    }

    /// Starts the waking process and performs bitwise or with the given value.
    fn start_waking(&self, to_poll: u8, waking: u8) -> u8 {
        self.state.fetch_or(to_poll | waking, Ordering::SeqCst)
    }

    /// Toggles state to non-waking, allowing to start polling.
    fn stop_waking(&self, waking: u8) -> u8 {
        self.state.fetch_and(!waking, Ordering::SeqCst)
    }

    /// Sets current state to `!POLLING`, allowing to use wakers.
    fn stop_polling(&self, to_poll: u8) -> u8 {
        self.state
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |state| {
                Some((state | to_poll) & !POLLING)
            })
            .unwrap()
    }
}

/// Waker which will update `poll_state` with `need_to_poll` value on
/// `wake_by_ref` call and then, if there is a need, call `inner_waker`.
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

    // Flags state that walking is started for the walker with the given value.
    fn start_waking(&self) -> u8 {
        self.poll_state.start_waking(self.need_to_poll, self.need_to_poll << 3)
    }

    // Flags state that walking is finished for the walker with the given value.
    fn stop_waking(&self) -> u8 {
        self.poll_state.stop_waking(self.need_to_poll << 3)
    }
}

impl ArcWake for InnerWaker {
    fn wake_by_ref(self_arc: &Arc<Self>) {
        let poll_state_value = self_arc.start_waking();

        // Only call waker if stream isn't being polled because of safety reasons.
        // Waker will be called at the end of polling if state was changed.
        if poll_state_value & POLLING == NONE {
            if let Some(inner_waker) =
                unsafe { self_arc.inner_waker.get().as_ref().cloned().flatten() }
            {
                // First, stop waking to allow polling stream
                self_arc.stop_waking();
                // Wake inner waker
                inner_waker.wake();
                return;
            }
        }

        self_arc.stop_waking();
    }
}

pin_project! {
    /// Future which contains optional stream. If it's `Some`, it will attempt
    /// to call `poll_next` on it, returning `Some((item, next_item_fut))` in
    /// case of `Poll::Ready(Some(...))` or `None` in case of `Poll::Ready(None)`.
    /// If `poll_next` will return `Poll::Pending`, it will be forwared to
    /// the future, and current task will be notified by waker.
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

impl<St: Stream> Future for PollStreamFut<St> {
    type Output = Option<(St::Item, PollStreamFut<St>)>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut stream = self.project().stream;

        let item = if let Some(stream) = stream.as_mut().as_pin_mut() {
            ready!(stream.poll_next(cx))
        } else {
            None
        };

        Poll::Ready(
            item.map(|item| {
                (item, PollStreamFut::new(unsafe { stream.get_unchecked_mut().take() }))
            }),
        )
    }
}

pin_project! {
    /// Stream for the [`flatten_unordered`](super::StreamExt::flatten_unordered)
    /// method.
    #[must_use = "streams do nothing unless polled"]
    pub struct FlattenUnordered<St, U> {
        #[pin]
        inner_streams: FuturesUnordered<PollStreamFut<U>>,
        #[pin]
        stream: St,
        poll_state: SharedPollState,
        limit: Option<NonZeroUsize>,
        is_stream_done: bool,
        inner_streams_waker: Arc<InnerWaker>,
        stream_waker: Arc<InnerWaker>,
    }
}

impl<St> fmt::Debug for FlattenUnordered<St, St::Item>
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

impl<St> FlattenUnordered<St, St::Item>
where
    St: Stream,
    St::Item: Stream,
{
    pub(super) fn new(stream: St, limit: Option<usize>) -> FlattenUnordered<St, St::Item> {
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

    /// Checks if current `inner_streams` size is less than optional limit.
    fn is_exceeded_limit(&self) -> bool {
        self.limit.map(|limit| self.inner_streams.len() >= limit.get()).unwrap_or(false)
    }

    delegate_access_inner!(stream, St, ());
}

impl<St> FusedStream for FlattenUnordered<St, St::Item>
where
    St: FusedStream,
    St::Item: FusedStream,
{
    fn is_terminated(&self) -> bool {
        self.stream.is_terminated() && self.inner_streams.is_empty()
    }
}

impl<St> Stream for FlattenUnordered<St, St::Item>
where
    St: Stream,
    St::Item: Stream,
{
    type Item = <St::Item as Stream>::Item;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut next_item = None;
        let mut need_to_poll_next = NONE;
        let limit_exceeded = self.is_exceeded_limit();

        let mut this = self.as_mut().project();

        let mut poll_state_value = match this.poll_state.start_polling() {
            Some(value) => value,
            _ => {
                // Waker was called, just wait for the next poll
                return Poll::Pending;
            }
        };

        let mut polling_with_two_wakers =
            !limit_exceeded && poll_state_value & NEED_TO_POLL_ALL == NEED_TO_POLL_ALL;

        if poll_state_value & NEED_TO_POLL_STREAM != NONE {
            if !limit_exceeded && !*this.is_stream_done {
                match if polling_with_two_wakers {
                    // Safety: now state is `POLLING`.
                    let waker = unsafe { InnerWaker::replace_waker(this.stream_waker, cx) };
                    let mut cx = Context::from_waker(&waker);
                    this.stream.as_mut().poll_next(&mut cx)
                } else {
                    this.stream.as_mut().poll_next(cx)
                } {
                    Poll::Ready(Some(inner_stream)) => {
                        this.inner_streams.as_mut().push(PollStreamFut::new(inner_stream));
                        need_to_poll_next |= NEED_TO_POLL_STREAM;
                        // Polling inner streams in current iteration with the same context
                        // is ok because we already received `Poll::Ready` from
                        // stream
                        poll_state_value |= NEED_TO_POLL_INNER_STREAMS;
                        polling_with_two_wakers = false;
                        *this.is_stream_done = false;
                    }
                    Poll::Ready(None) => {
                        // Polling inner streams in current iteration with the same context
                        // is ok because we already received `Poll::Ready` from
                        // stream
                        polling_with_two_wakers = false;
                        *this.is_stream_done = true;
                    }
                    Poll::Pending => {
                        if !polling_with_two_wakers {
                            need_to_poll_next |= NEED_TO_POLL_STREAM;
                        }
                        *this.is_stream_done = false;
                    }
                }
            } else {
                need_to_poll_next |= NEED_TO_POLL_STREAM;
            }
        }

        if poll_state_value & NEED_TO_POLL_INNER_STREAMS != NONE {
            match if polling_with_two_wakers {
                // Safety: now state is `POLLING`.
                let waker = unsafe { InnerWaker::replace_waker(this.inner_streams_waker, cx) };
                let mut cx = Context::from_waker(&waker);
                this.inner_streams.as_mut().poll_next(&mut cx)
            } else {
                this.inner_streams.as_mut().poll_next(cx)
            } {
                Poll::Ready(Some(Some((item, next_item_fut)))) => {
                    this.inner_streams.as_mut().push(next_item_fut);
                    next_item = Some(item);
                    need_to_poll_next |= NEED_TO_POLL_INNER_STREAMS;
                }
                Poll::Ready(Some(None)) => {
                    need_to_poll_next |= NEED_TO_POLL_ALL;
                }
                Poll::Pending => {
                    if !polling_with_two_wakers {
                        need_to_poll_next |= NEED_TO_POLL_INNER_STREAMS;
                    }
                }
                Poll::Ready(None) => {
                    need_to_poll_next &= !NEED_TO_POLL_INNER_STREAMS;
                }
            }
        }

        poll_state_value = this.poll_state.stop_polling(need_to_poll_next);
        let is_done = *this.is_stream_done && this.inner_streams.is_empty();

        if next_item.is_some() || is_done {
            Poll::Ready(next_item)
        } else {
            if poll_state_value & NEED_TO_POLL_ALL != NONE
                || !self.is_exceeded_limit() && need_to_poll_next & NEED_TO_POLL_STREAM != NONE
            {
                cx.waker().wake_by_ref();
            }

            Poll::Pending
        }
    }
}

// Forwarding impl of Sink from the underlying stream
#[cfg(feature = "sink")]
impl<S, Item> Sink<Item> for FlattenUnordered<S, S::Item>
where
    S: Stream + Sink<Item>,
    S::Item: Stream,
{
    type Error = S::Error;

    delegate_sink!(stream, Item);
}
