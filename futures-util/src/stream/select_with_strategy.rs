use super::assert_stream;
use core::marker::PhantomData;
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

/// Which streams are known to have finished?
#[derive(PartialEq, Debug, Eq, Clone, Copy)]
pub enum ClosedStreams {
    /// Neither stream has finished
    None,
    /// The left stream has finished
    Left,
    /// The right stream has finished
    Right,
    /// Both streams have finished
    Both,
}

/// A trait for chosing to close the stream before both streams
/// have finished.
pub trait ExitStrategy {
    /// Chose whether this stream is terminated based on the termination state of its
    /// substreams.
    fn is_terminated(closed_streams: ClosedStreams) -> bool;
}

impl ClosedStreams {
    fn finish(&mut self, ps: PollNext) {
        match (&self, ps) {
            (ClosedStreams::None, PollNext::Left) => {
                *self = ClosedStreams::Left;
            }
            (ClosedStreams::None, PollNext::Right) => {
                *self = ClosedStreams::Right;
            }
            (ClosedStreams::Left, PollNext::Right) | (ClosedStreams::Right, PollNext::Left) => {
                *self = ClosedStreams::Both;
            }
            _ => {}
        }
    }
}

pin_project! {
    /// Stream for the [`select_with_strategy()`] function. See function docs for details.
    #[must_use = "streams do nothing unless polled"]
    #[project = SelectWithStrategyProj]
    pub struct SelectWithStrategy<St1, St2, Clos, State, Exit> {
        #[pin]
        stream1: St1,
        #[pin]
        stream2: St2,
        closed_streams: ClosedStreams,
        state: State,
        clos: Clos,
        _marker: PhantomData<Exit>,
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
/// use futures::stream::{ repeat, select_with_strategy, PollNext, StreamExt,
///     ClosedStreams, ExitStrategy, SelectWithStrategy };
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
/// struct ExitWhenBothFinished {}
///
/// impl ExitStrategy for ExitWhenBothFinished {
///     fn is_terminated(closed_streams: ClosedStreams) -> bool {
///         match closed_streams {
///             ClosedStreams::Both => true,
///             _ => false,
///         }
///     }
/// }
///
/// let mut out: SelectWithStrategy<_, _, _, _, ExitWhenBothFinished> = select_with_strategy(left, right, prio_left);
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
/// Note: these special cases are provided by [`futures-util::stream::select`].
///
/// ```rust
/// # futures::executor::block_on(async {
/// use futures::stream::{ repeat, select_with_strategy, FusedStream, PollNext, StreamExt,
///     ClosedStreams, ExitStrategy, SelectWithStrategy };
///
/// struct ExitWhenBothFinished {}
///
/// impl ExitStrategy for ExitWhenBothFinished {
///     fn is_terminated(closed_streams: ClosedStreams) -> bool {
///         match closed_streams {
///             ClosedStreams::Both => true,
///             _ => false,
///         }
///     }
/// }
///
/// struct ExitWhenEitherFinished {}
///
/// impl ExitStrategy for ExitWhenEitherFinished {
///     fn is_terminated(closed_streams: ClosedStreams) -> bool {
///         match closed_streams {
///             ClosedStreams::None => false,
///             _ => true,
///         }
///     }
/// }
///
/// // Finishes when both streams finish
/// {
///     let left = repeat(1).take(10);
///     let right = repeat(2);
///
///     let rrobin = |last: &mut PollNext| last.toggle();
///
///     let mut out: SelectWithStrategy<_, _, _, _, ExitWhenBothFinished> = select_with_strategy(left, right, rrobin);
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
///     let mut out: SelectWithStrategy<_, _, _, _, ExitWhenEitherFinished>  = select_with_strategy(left, right, rrobin);
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
pub fn select_with_strategy<St1, St2, Clos, State, Exit>(
    stream1: St1,
    stream2: St2,
    which: Clos,
) -> SelectWithStrategy<St1, St2, Clos, State, Exit>
where
    St1: Stream,
    St2: Stream<Item = St1::Item>,
    Clos: FnMut(&mut State) -> PollNext,
    State: Default,
    Exit: ExitStrategy,
{
    assert_stream::<St1::Item, _>(SelectWithStrategy {
        stream1,
        stream2,
        state: Default::default(),
        clos: which,
        closed_streams: ClosedStreams::None,
        _marker: PhantomData,
    })
}

impl<St1, St2, Clos, State, Exit> SelectWithStrategy<St1, St2, Clos, State, Exit> {
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

impl<St1, St2, Clos, State, Exit> FusedStream for SelectWithStrategy<St1, St2, Clos, State, Exit>
where
    St1: Stream,
    St2: Stream<Item = St1::Item>,
    Clos: FnMut(&mut State) -> PollNext,
    Exit: ExitStrategy,
{
    fn is_terminated(&self) -> bool {
        Exit::is_terminated(self.closed_streams)
    }
}

#[inline]
fn poll_side<St1, St2, Clos, State, Exit>(
    select: &mut SelectWithStrategyProj<'_, St1, St2, Clos, State, Exit>,
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
fn poll_inner<St1, St2, Clos, State, Exit>(
    select: &mut SelectWithStrategyProj<'_, St1, St2, Clos, State, Exit>,
    side: PollNext,
    cx: &mut Context<'_>,
) -> Poll<Option<St1::Item>>
where
    St1: Stream,
    St2: Stream<Item = St1::Item>,
    Exit: ExitStrategy,
{
    match poll_side(select, side, cx) {
        Poll::Ready(Some(item)) => return Poll::Ready(Some(item)),
        Poll::Ready(None) => {
            select.closed_streams.finish(side);
            if Exit::is_terminated(*select.closed_streams) {
                return Poll::Ready(None);
            }
        }
        Poll::Pending => (),
    };
    let other = side.other();
    match poll_side(select, other, cx) {
        Poll::Ready(None) => {
            select.closed_streams.finish(other);
            Poll::Ready(None)
        }
        a => a,
    }
}

impl<St1, St2, Clos, State, Exit> Stream for SelectWithStrategy<St1, St2, Clos, State, Exit>
where
    St1: Stream,
    St2: Stream<Item = St1::Item>,
    Clos: FnMut(&mut State) -> PollNext,
    Exit: ExitStrategy,
{
    type Item = St1::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<St1::Item>> {
        if self.is_terminated() {
            return Poll::Ready(None);
        }

        let mut this = self.project();

        match this.closed_streams {
            ClosedStreams::None => {
                let next_side = (this.clos)(this.state);
                poll_inner(&mut this, next_side, cx)
            }
            ClosedStreams::Left => match this.stream2.poll_next(cx) {
                Poll::Ready(None) => {
                    *this.closed_streams = ClosedStreams::Both;
                    Poll::Ready(None)
                }
                a => a,
            },
            ClosedStreams::Right => match this.stream1.poll_next(cx) {
                Poll::Ready(None) => {
                    *this.closed_streams = ClosedStreams::Both;
                    Poll::Ready(None)
                }
                a => a,
            },
            ClosedStreams::Both => Poll::Ready(None),
        }
    }
}

impl<St1, St2, Clos, State, Exit> fmt::Debug for SelectWithStrategy<St1, St2, Clos, State, Exit>
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
