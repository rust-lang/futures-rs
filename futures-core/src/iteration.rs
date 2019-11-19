//! Utilities for implementing iterative polling in a cooperative way.

use core::num::NonZeroU32;

/// Iteration policy checker for eager polling loops.
pub trait LoopPolicy {
    /// State for checking iterations over the lifetime of a polling loop.
    type State;

    /// Called before entering the loop to initialize state for
    /// iteration checks.
    fn enter(&mut self) -> Self::State;

    /// Called at the end of each iteration to update the check state
    /// and decide whether the poll function should yield, that is,
    /// wake up the task and return
    /// [`Pending`](core::task::poll::Poll::Pending).
    fn yield_check(&mut self, state: &mut Self::State) -> bool;
}

/// An unlimited iteration policy.
///
/// The [`LoopPolicy`] implementation for a value of this token type bypasses
/// any checking on iterations of a polling loop, allowing it to continue
/// indefinitely until ended by logic in the loop body (normally when
/// an asynchronous source is pending or the poll resolves with an output
/// value).
#[derive(Debug)]
pub struct Unlimited {}

impl LoopPolicy for Unlimited {
    type State = ();

    #[inline]
    fn enter(&mut self) {}

    #[inline]
    fn yield_check(&mut self, _: &mut ()) -> bool {
        false
    }
}

/// An iteration policy with a limit on the number of consecutive iterations.
///
/// The [`LoopPolicy`] implementation of this type runs a counter on the number
/// of iterations and, when the given limit is reached, instructs the
/// [`yield_check`](LoopPolicy::yield_check) caller to yield.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Limit(NonZeroU32);

impl Limit {
    /// Creates a policy checker instance with the given limit.
    #[inline]
    pub const fn new(value: NonZeroU32) -> Self {
        Limit(value)
    }
}

impl LoopPolicy for Limit {
    type State = u32;

    #[inline]
    fn enter(&mut self) -> u32 {
        self.0.into()
    }

    #[inline]
    fn yield_check(&mut self, counter: &mut u32) -> bool {
        *counter -= 1;
        *counter == 0
    }
}
