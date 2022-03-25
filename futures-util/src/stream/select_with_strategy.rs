use super::assert_stream;
use core::{fmt, pin::Pin};
use futures_core::stream::{FusedStream, Stream};
use futures_core::task::{Context, Poll};
use pin_project_lite::pin_project;

/// Type to tell [`SelectWithStrategy`] which stream to poll next.
#[derive(Debug, PartialEq, Eq, Copy, Clone, Hash)]
pub enum PollNext {
    /// Poll the first stream.
    Left,
    /// Poll the second stream.
    Right,
}

impl PollNext {
    /// Toggle the value and return the old one.
    #[must_use]
    pub fn toggle(&mut self) -> Self {
        let old = *self;
        *self = self.other();
        old
    }

    fn other(&self) -> PollNext {
        match self {
            PollNext::Left => PollNext::Right,
            PollNext::Right => PollNext::Left,
        }
    }
}

impl Default for PollNext {
    fn default() -> Self {
        PollNext::Left
    }
}

#[derive(PartialEq, Eq, Clone, Copy)]
enum InternalState {
    Start,
    LeftFinished,
    RightFinished,
    BothFinished,
}

impl InternalState {
    fn finish(&mut self, ps: PollNext) {
        match (&self, ps) {
            (InternalState::Start, PollNext::Left) => {
                *self = InternalState::LeftFinished;
            }
            (InternalState::Start, PollNext::Right) => {
                *self = InternalState::RightFinished;
            }
            (InternalState::LeftFinished, PollNext::Right)
            | (InternalState::RightFinished, PollNext::Left) => {
                *self = InternalState::BothFinished;
            }
            _ => {}
        }
    }
}

/// Decides whether to exit when both streams are completed, or only one
/// is completed. If you need to exit when a specific stream has finished,
/// feel free to add a case here.
#[derive(Clone, Copy, Debug)]
pub enum ExitStrategy {
    /// Select stream finishes when both substreams finish
    WhenBothFinish,
    /// Select stream finishes when either substream finishes
    WhenEitherFinish,
}

impl ExitStrategy {
    #[inline]
    fn is_finished(self, state: InternalState) -> bool {
        match (state, self) {
            (InternalState::BothFinished, _) => true,
            (InternalState::Start, ExitStrategy::WhenEitherFinish) => false,
            (_, ExitStrategy::WhenBothFinish) => false,
            _ => true,
        }
    }
}

pin_project! {
    /// Stream for the [`select_with_strategy()`] function. See function docs for details.
    #[must_use = "streams do nothing unless polled"]
    #[project = SelectWithStrategyProj]
    pub struct SelectWithStrategy<St1, St2, Clos, State> {
        #[pin]
        stream1: St1,
        #[pin]
        stream2: St2,
        internal_state: InternalState,
        state: State,
        clos: Clos,
        exit_strategy: ExitStrategy,
    }
}

/// This function will attempt to pull items from both streams. You provide a
/// closure to tell [`SelectWithStrategy`] which stream to poll. The closure can
/// store state on `SelectWithStrategy` to which it will receive a `&mut` on every
/// invocation. This allows basing the strategy on prior choices.
///
/// After one of the two input streams completes, the remaining one will be
/// polled exclusively. The returned stream completes when both input
/// streams have completed.
///
/// Note that this function consumes both streams and returns a wrapped
/// version of them.
///
/// ## Examples
///
/// ### Priority
/// This example shows how to always prioritize the left stream.
///
/// ```rust
/// # futures::executor::block_on(async {
/// use futures::stream::{ repeat, select_with_strategy, PollNext, StreamExt, ExitStrategy };
///
/// let left = repeat(1);
/// let right = repeat(2);
///
/// // We don't need any state, so let's make it an empty tuple.
/// // We must provide some type here, as there is no way for the compiler
/// // to infer it. As we don't need to capture variables, we can just
/// // use a function pointer instead of a closure.
/// fn prio_left(_: &mut ()) -> PollNext { PollNext::Left }
///
/// let mut out = select_with_strategy(left, right, prio_left, ExitStrategy::WhenBothFinish);
///
/// for _ in 0..100 {
///     // Whenever we poll out, we will always get `1`.
///     assert_eq!(1, out.select_next_some().await);
/// }
/// # });
/// ```
///
/// ### Round Robin
/// This example shows how to select from both streams round robin.
/// Note: this special case is provided by [`futures-util::stream::select`].
///
/// ```rust
/// # futures::executor::block_on(async {
/// use futures::stream::{ repeat, select_with_strategy, FusedStream, PollNext, StreamExt, ExitStrategy };
///
/// // Finishes when both streams finish
/// {
///     let left = repeat(1).take(10);
///     let right = repeat(2);
///
///     let rrobin = |last: &mut PollNext| last.toggle();
///
///     let mut out = select_with_strategy(left, right, rrobin, ExitStrategy::WhenBothFinish);
///
///     for _ in 0..10 {
///         // We should be alternating now.
///         assert_eq!(1, out.select_next_some().await);
///         assert_eq!(2, out.select_next_some().await);
///     }
///     for _ in 0..100 {
///         // First stream has finished
///         assert_eq!(2, out.select_next_some().await);
///     }
///     assert!(!out.is_terminated());
/// }
///
/// // Finishes when either stream finishes
/// {
///     let left = repeat(1).take(10);
///     let right = repeat(2);
///
///     let rrobin = |last: &mut PollNext| last.toggle();
///
///     let mut out = select_with_strategy(left, right, rrobin, ExitStrategy::WhenEitherFinish);
///
///     for _ in 0..10 {
///         // We should be alternating now.
///         assert_eq!(1, out.select_next_some().await);
///         assert_eq!(2, out.select_next_some().await);
///     }
///     assert_eq!(None, out.next().await);
///     assert!(out.is_terminated());
/// }
/// # });
/// ```
///
pub fn select_with_strategy<St1, St2, Clos, State>(
    stream1: St1,
    stream2: St2,
    which: Clos,
    exit_strategy: ExitStrategy,
) -> SelectWithStrategy<St1, St2, Clos, State>
where
    St1: Stream,
    St2: Stream<Item = St1::Item>,
    Clos: FnMut(&mut State) -> PollNext,
    State: Default,
{
    assert_stream::<St1::Item, _>(SelectWithStrategy {
        stream1,
        stream2,
        state: Default::default(),
        internal_state: InternalState::Start,
        clos: which,
        exit_strategy,
    })
}

impl<St1, St2, Clos, State> SelectWithStrategy<St1, St2, Clos, State> {
    /// Acquires a reference to the underlying streams that this combinator is
    /// pulling from.
    pub fn get_ref(&self) -> (&St1, &St2) {
        (&self.stream1, &self.stream2)
    }

    /// Acquires a mutable reference to the underlying streams that this
    /// combinator is pulling from.
    ///
    /// Note that care must be taken to avoid tampering with the state of the
    /// stream which may otherwise confuse this combinator.
    pub fn get_mut(&mut self) -> (&mut St1, &mut St2) {
        (&mut self.stream1, &mut self.stream2)
    }

    /// Acquires a pinned mutable reference to the underlying streams that this
    /// combinator is pulling from.
    ///
    /// Note that care must be taken to avoid tampering with the state of the
    /// stream which may otherwise confuse this combinator.
    pub fn get_pin_mut(self: Pin<&mut Self>) -> (Pin<&mut St1>, Pin<&mut St2>) {
        let this = self.project();
        (this.stream1, this.stream2)
    }

    /// Consumes this combinator, returning the underlying streams.
    ///
    /// Note that this may discard intermediate state of this combinator, so
    /// care should be taken to avoid losing resources when this is called.
    pub fn into_inner(self) -> (St1, St2) {
        (self.stream1, self.stream2)
    }
}

impl<St1, St2, Clos, State> FusedStream for SelectWithStrategy<St1, St2, Clos, State>
where
    St1: Stream,
    St2: Stream<Item = St1::Item>,
    Clos: FnMut(&mut State) -> PollNext,
{
    fn is_terminated(&self) -> bool {
        self.exit_strategy.is_finished(self.internal_state)
    }
}

#[inline]
fn poll_side<St1, St2, Clos, State>(
    select: &mut SelectWithStrategyProj<'_, St1, St2, Clos, State>,
    side: PollNext,
    cx: &mut Context<'_>,
) -> Poll<Option<St1::Item>>
where
    St1: Stream,
    St2: Stream<Item = St1::Item>,
{
    match side {
        PollNext::Left => select.stream1.as_mut().poll_next(cx),
        PollNext::Right => select.stream2.as_mut().poll_next(cx),
    }
}

#[inline]
fn poll_inner<St1, St2, Clos, State>(
    select: &mut SelectWithStrategyProj<'_, St1, St2, Clos, State>,
    side: PollNext,
    cx: &mut Context<'_>,
    exit_strat: ExitStrategy,
) -> Poll<Option<St1::Item>>
where
    St1: Stream,
    St2: Stream<Item = St1::Item>,
{
    match poll_side(select, side, cx) {
        Poll::Ready(Some(item)) => return Poll::Ready(Some(item)),
        Poll::Ready(None) => {
            select.internal_state.finish(side);
            if exit_strat.is_finished(*select.internal_state) {
                return Poll::Ready(None);
            }
        }
        Poll::Pending => (),
    };
    let other = side.other();
    match poll_side(select, other, cx) {
        Poll::Ready(None) => {
            select.internal_state.finish(other);
            Poll::Ready(None)
        }
        a => a,
    }
}

impl<St1, St2, Clos, State> Stream for SelectWithStrategy<St1, St2, Clos, State>
where
    St1: Stream,
    St2: Stream<Item = St1::Item>,
    Clos: FnMut(&mut State) -> PollNext,
{
    type Item = St1::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<St1::Item>> {
        let mut this = self.project();
        let exit_strategy: ExitStrategy = *this.exit_strategy;

        if exit_strategy.is_finished(*this.internal_state) {
            return Poll::Ready(None);
        }

        match this.internal_state {
            InternalState::Start => {
                let next_side = (this.clos)(this.state);
                poll_inner(&mut this, next_side, cx, exit_strategy)
            }
            InternalState::LeftFinished => match this.stream2.poll_next(cx) {
                Poll::Ready(None) => {
                    *this.internal_state = InternalState::BothFinished;
                    Poll::Ready(None)
                }
                a => a,
            },
            InternalState::RightFinished => match this.stream1.poll_next(cx) {
                Poll::Ready(None) => {
                    *this.internal_state = InternalState::BothFinished;
                    Poll::Ready(None)
                }
                a => a,
            },
            InternalState::BothFinished => Poll::Ready(None),
        }
    }
}

impl<St1, St2, Clos, State> fmt::Debug for SelectWithStrategy<St1, St2, Clos, State>
where
    St1: fmt::Debug,
    St2: fmt::Debug,
    State: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SelectWithStrategy")
            .field("stream1", &self.stream1)
            .field("stream2", &self.stream2)
            .field("state", &self.state)
            .finish()
    }
}
