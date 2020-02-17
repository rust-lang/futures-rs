use super::Map;
use crate::stream::FuturesUnordered;
use core::fmt;
use core::num::NonZeroUsize;
use core::pin::Pin;
use core::sync::atomic::{Ordering, AtomicU8};
use alloc::sync::Arc;
use futures_core::future::Future;
use futures_core::stream::FusedStream;
use futures_core::stream::Stream;
use futures_core::task::{Context, Poll, Waker};
#[cfg(feature = "sink")]
use futures_sink::Sink;
use futures_task::{waker, ArcWake};
use pin_utils::{unsafe_pinned, unsafe_unpinned};

/// Indicates that there is nothing to poll and stream isn't polled at the
/// moment.
const NONE: u8 = 0;

/// Indicates that `futures` need to be polled.
const NEED_TO_POLL_FUTURES: u8 = 0b1;

/// Indicates that `stream` needs to be polled.
const NEED_TO_POLL_STREAM: u8 = 0b10;

/// Indicates that it needs to poll something.
const NEED_TO_POLL: u8 = NEED_TO_POLL_FUTURES | NEED_TO_POLL_STREAM;

/// Indicates that current stream is polled at the moment.
const POLLING: u8 = 0b100;

/// State which used to determine what needs to be polled,
/// and are we polling stream at the moment or not.
#[derive(Clone, Debug)]
struct SharedPollState {
    state: Arc<AtomicU8>,
}

impl SharedPollState {
    /// Constructs new `SharedPollState` with given state.
    fn new(state: u8) -> Self {
        Self {
            state: Arc::new(AtomicU8::new(state)),
        }
    }

    /// Swaps state with `POLLING`, returning previous state.
    fn begin_polling(&self) -> u8 {
        self.state.swap(POLLING, Ordering::AcqRel)
    }

    /// Performs bitwise or with `to_poll` and given state, returning
    /// previous state.
    fn set_or(&self, to_poll: u8) -> u8 {
        self.state.fetch_or(to_poll, Ordering::AcqRel)
    }

    /// Performs bitwise or with `to_poll` and current state, stores result
    /// with non-`POLLING` state, and returns disjunction result.
    fn end_polling(&self, to_poll: u8) -> u8 {
        let to_poll = to_poll | self.state.load(Ordering::Acquire);
        self.state.store(to_poll & !POLLING, Ordering::Release);
        to_poll
    }
}

/// Waker which will update `poll_state` with `need_to_poll` value on
/// `wake_by_ref` call and then, if there is a need, call `inner_waker`.
struct PollWaker {
    inner_waker: Waker,
    poll_state: SharedPollState,
    need_to_poll: u8,
}

impl ArcWake for PollWaker {
    fn wake_by_ref(self_arc: &Arc<Self>) {
        let poll_state_value = self_arc.poll_state.set_or(self_arc.need_to_poll);
        // Only call waker if stream isn't polled because it will called at the end
        // of polling if it needs to poll something.
        if poll_state_value & POLLING == NONE {
            self_arc.inner_waker.wake_by_ref();
        }
    }
}

/// Future which contains optional stream. If it's `Some`, it will attempt
/// to call `poll_next` on it, returning `Some((item, next_item_fut))` in
/// case of `Poll::Ready(Some(...))` or `None` in case of `Poll::Ready(None)`.
/// If `poll_next` will return `Poll::Pending`, it will be forwared to
/// the future, and current task will be notified by waker.
#[must_use = "futures do nothing unless you `.await` or poll them"]
struct PollStreamFut<St> {
    stream: Option<St>,
}

impl<St> PollStreamFut<St> {
    unsafe_pinned!(stream: Option<St>);

    /// Constructs new `PollStreamFut` using given `stream`.
    fn new(stream: impl Into<Option<St>>) -> Self {
        Self {
            stream: stream.into(),
        }
    }
}

impl<St: Stream + Unpin> Unpin for PollStreamFut<St> {}

impl<St: Stream> Future for PollStreamFut<St> {
    type Output = Option<(St::Item, PollStreamFut<St>)>;

    fn poll(mut self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Self::Output> {
        let item = if let Some(stream) = self.as_mut().stream().as_pin_mut() {
            ready!(stream.poll_next(ctx))
        } else {
            None
        };

        Poll::Ready(item.map(|item| {
            (
                item,
                PollStreamFut::new(unsafe { self.get_unchecked_mut().stream.take() }),
            )
        }))
    }
}

/// Stream for the [`flat_map_unordered`](super::StreamExt::flat_map_unordered)
/// method.
#[must_use = "streams do nothing unless polled"]
pub struct FlatMapUnordered<St: Stream, U: Stream, F: FnMut(St::Item) -> U> {
    poll_state: SharedPollState,
    futures: FuturesUnordered<PollStreamFut<U>>,
    stream: Map<St, F>,
    limit: Option<NonZeroUsize>,
    is_stream_done: bool,
}

impl<St, U, F> Unpin for FlatMapUnordered<St, U, F>
where
    St: Stream + Unpin,
    U: Stream + Unpin,
    F: FnMut(St::Item) -> U,
{
}

impl<St, U, F> fmt::Debug for FlatMapUnordered<St, U, F>
where
    St: Stream + fmt::Debug,
    U: Stream + fmt::Debug,
    F: FnMut(St::Item) -> U,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FlatMapUnordered")
            .field("poll_state", &self.poll_state)
            .field("futures", &self.futures)
            .field("limit", &self.limit)
            .field("stream", &self.stream)
            .field("is_stream_done", &self.is_stream_done)
            .finish()
    }
}

impl<St, U, F> FlatMapUnordered<St, U, F>
where
    St: Stream,
    U: Stream,
    F: FnMut(St::Item) -> U,
{
    unsafe_pinned!(futures: FuturesUnordered<PollStreamFut<U>>);
    unsafe_pinned!(stream: Map<St, F>);
    unsafe_unpinned!(is_stream_done: bool);
    unsafe_unpinned!(limit: Option<NonZeroUsize>);
    unsafe_unpinned!(poll_state: SharedPollState);

    pub(super) fn new(stream: St, limit: Option<usize>, f: F) -> FlatMapUnordered<St, U, F> {
        FlatMapUnordered {
            // Because to create first future, it needs to get inner
            // stream from `stream`
            poll_state: SharedPollState::new(NEED_TO_POLL_STREAM),
            futures: FuturesUnordered::new(),
            stream: Map::new(stream, f),
            is_stream_done: false,
            limit: limit.and_then(NonZeroUsize::new),
        }
    }

    /// Acquires a reference to the underlying stream that this combinator is
    /// pulling from.
    pub fn get_ref(&self) -> &St {
        self.stream.get_ref()
    }

    /// Acquires a mutable reference to the underlying stream that this
    /// combinator is pulling from.
    ///
    /// Note that care must be taken to avoid tampering with the state of the
    /// stream which may otherwise confuse this combinator.
    pub fn get_mut(&mut self) -> &mut St {
        self.stream.get_mut()
    }

    /// Acquires a pinned mutable reference to the underlying stream that this
    /// combinator is pulling from.
    ///
    /// Note that care must be taken to avoid tampering with the state of the
    /// stream which may otherwise confuse this combinator.
    pub fn get_pin_mut(self: Pin<&mut Self>) -> Pin<&mut St> {
        self.stream().get_pin_mut()
    }

    /// Consumes this combinator, returning the underlying stream.
    ///
    /// Note that this may discard intermediate state of this combinator, so
    /// care should be taken to avoid losing resources when this is called.
    pub fn into_inner(self) -> St {
        self.stream.into_inner()
    }

    /// Creates waker with given `need_to_poll` value, which will be used to
    /// update poll state on `wake_by_ref` call.
    fn create_waker(&self, inner_waker: Waker, need_to_poll: u8) -> Waker {
        waker(Arc::new(PollWaker {
            inner_waker,
            poll_state: self.poll_state.clone(),
            need_to_poll,
        }))
    }

    /// Creates special waker for polling stream which will set poll state
    /// to poll `stream` on `wake_by_ref` call. Use only if you need several
    /// contexts.
    fn create_poll_stream_waker(&self, ctx: &Context<'_>) -> Waker {
        self.create_waker(ctx.waker().clone(), NEED_TO_POLL_STREAM)
    }

    /// Creates special waker for polling futures which willset poll state
    /// to poll `futures` on `wake_by_ref` call. Use only if you need several
    /// contexts.
    fn create_poll_futures_waker(&self, ctx: &Context<'_>) -> Waker {
        self.create_waker(ctx.waker().clone(), NEED_TO_POLL_FUTURES)
    }

    /// Checks if current `futures` size is less than optional limit.
    fn not_exceeded_limit(&self) -> bool {
        self.limit
            .map(|limit| self.futures.len() < limit.get())
            .unwrap_or(true)
    }
}

impl<St, U, F> FusedStream for FlatMapUnordered<St, U, F>
where
    St: FusedStream,
    U: FusedStream,
    F: FnMut(St::Item) -> U,
{
    fn is_terminated(&self) -> bool {
        self.futures.is_empty() && self.stream.is_terminated()
    }
}

impl<St, U, F> Stream for FlatMapUnordered<St, U, F>
where
    St: Stream,
    U: Stream,
    F: FnMut(St::Item) -> U,
{
    type Item = U::Item;

    fn poll_next(mut self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut poll_state_value = self.as_mut().poll_state().begin_polling();
        let mut next_item = None;
        let mut need_to_poll_next = NONE;
        let mut polling_with_two_wakers =
            poll_state_value & NEED_TO_POLL == NEED_TO_POLL && self.not_exceeded_limit();
        let mut stream_will_be_woken = false;
        let mut futures_will_be_woken = false;

        if poll_state_value & NEED_TO_POLL_STREAM != NONE {
            if self.not_exceeded_limit() {
                match if polling_with_two_wakers {
                    let waker = self.create_poll_stream_waker(ctx);
                    let mut ctx = Context::from_waker(&waker);
                    self.as_mut().stream().poll_next(&mut ctx)
                } else {
                    self.as_mut().stream().poll_next(ctx)
                } {
                    Poll::Ready(Some(inner_stream)) => {
                        self.as_mut().futures().push(PollStreamFut::new(inner_stream));
                        need_to_poll_next |= NEED_TO_POLL_STREAM;
                        // Polling futures in current iteration with the same context
                        // is ok because we already received `Poll::Ready` from
                        // stream
                        poll_state_value |= NEED_TO_POLL_FUTURES;
                        polling_with_two_wakers = false;
                    }
                    Poll::Ready(None) => {
                        *self.as_mut().is_stream_done() = true;
                        // Polling futures in current iteration with the same context
                        // is ok because we already received `Poll::Ready` from
                        // stream
                        polling_with_two_wakers = false;
                    }
                    Poll::Pending => {
                        stream_will_be_woken = true;
                        if !polling_with_two_wakers {
                            need_to_poll_next |= NEED_TO_POLL_STREAM;
                        }
                    }
                }
            } else {
                need_to_poll_next |= NEED_TO_POLL_STREAM;
            }
        }

        if poll_state_value & NEED_TO_POLL_FUTURES != NONE {
            match if polling_with_two_wakers {
                let waker = self.create_poll_futures_waker(ctx);
                let mut ctx = Context::from_waker(&waker);
                self.as_mut().futures().poll_next(&mut ctx)
            } else {
                self.as_mut().futures().poll_next(ctx)
            } {
                Poll::Ready(Some(Some((item, next_item_fut)))) => {
                    self.as_mut().futures().push(next_item_fut);
                    next_item = Some(item);
                    need_to_poll_next |= NEED_TO_POLL_FUTURES;
                }
                Poll::Ready(Some(None)) => {
                    need_to_poll_next |= NEED_TO_POLL_FUTURES;
                }
                Poll::Pending => {
                    futures_will_be_woken = true;
                    if !polling_with_two_wakers {
                        need_to_poll_next |= NEED_TO_POLL_FUTURES;
                    }
                }
                _ => {
                    need_to_poll_next &= !NEED_TO_POLL_FUTURES;
                }
            }
        }

        let poll_state_value = self.as_mut().poll_state().end_polling(need_to_poll_next);

        if poll_state_value & NEED_TO_POLL != NONE
            && (polling_with_two_wakers
                || poll_state_value & NEED_TO_POLL_FUTURES != NONE && !futures_will_be_woken
                    || poll_state_value & NEED_TO_POLL_STREAM != NONE && !stream_will_be_woken)
        {
            ctx.waker().wake_by_ref();
        }

        if next_item.is_some() || self.futures.is_empty() && self.is_stream_done {
            Poll::Ready(next_item)
        } else {
            Poll::Pending
        }
    }
}

// Forwarding impl of Sink from the underlying stream
#[cfg(feature = "sink")]
impl<S, U, F, Item> Sink<Item> for FlatMapUnordered<S, U, F>
where
    S: Stream + Sink<Item>,
    U: Stream,
    F: FnMut(S::Item) -> U,
{
    type Error = S::Error;

    delegate_sink!(stream, Item);
}
