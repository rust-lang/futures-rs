//! Utilities for implementing iterative polling in a cooperative way.

use core::mem;
use core::num::NonZeroU32;

/// Iteration policy checker for eager polling loops.
pub trait Policy {
    /// State for checking iterations over the lifetime of a polling loop.
    type State;

    /// Called before entering the loop to initialize state for
    /// iteration checks.
    fn begin(&mut self) -> Self::State;

    /// Called at the end of each iteration to update the check state
    /// and decide whether the poll function should yield, that is,
    /// wake up the task and return
    /// [`Pending`](core::task::poll::Poll::Pending).
    fn yield_check(&self, state: &mut Self::State) -> bool;

    /// Called when the loop ends before `self` has returned true from
    /// its [`yield_check`] method.
    /// `state` refers to the loop check state as created by [`begin`]
    /// and modified by `yield_check` over the iterations of the loop.
    /// Note that another iteration has been partially executed since the
    /// last call to `yield_check` when this method is called.
    ///
    /// [`begin`]: Policy::begin
    /// [`yield_check`]: Policy::yield_check
    ///
    /// The default implementation of this method does nothing.
    /// Some advanced `Policy` implementations may use this method to collect
    /// statistics or implement adaptive behaviors.
    fn when_ended_early(&mut self, #[allow(unused_variables)] state: &mut Self::State) {}

    /// Called immediately after `self` has returned true from its
    /// [`yield_check`] method, before returning from the poll function.
    /// `state` refers to the loop check state as it is left by the call
    /// to `yield_check`.
    ///
    /// [`begin`]: Policy::begin
    /// [`yield_check`]: Policy::yield_check
    ///
    /// The default implementation of this method does nothing.
    /// Some advanced `Policy` implementations may use this method to collect
    /// statistics or implement adaptive behaviors.
    fn when_yielded(&mut self, #[allow(unused_variables)] state: &mut Self::State) {}
}

/// A RAII guard around [`Policy`] and its state created to check iterations
/// of a polling loop.
///
/// This type is used by the [`poll_loop!`] macro, so the programmer of a
/// polling loop does not need to use it directly.
#[derive(Debug)]
pub struct LoopGuard<'a, P>
where
    P: ?Sized + Policy,
{
    policy: &'a mut P,
    state: P::State,
}

impl<'a, P> LoopGuard<'a, P>
where
    P: ?Sized + Policy,
{
    /// Wraps a lifetime-scoped guard around a reference to the policy
    /// checker object and initializes the iteration check state.
    pub fn new(policy: &'a mut P) -> Self {
        let state = policy.begin();
        LoopGuard { policy, state }
    }

    /// Calls the [`yield_check`](Policy::yield_check) method on the wrapped
    /// [`Policy`] reference with its internal state object.
    pub fn yield_check(&mut self) -> bool {
        self.policy.yield_check(&mut self.state)
    }

    /// Called immediately after [`yield_check`](Policy::yield_check)
    /// has returned `true`, to call [`when_yielded`](Policy::when_yielded)
    /// on the wrapped [`Policy`] reference with its internal state object.
    /// The normal destructor of `LoopGuard` is then bypassed.
    pub fn process_yield(mut self) {
        self.policy.when_yielded(&mut self.state);
        mem::forget(self);
    }
}

impl<P> Drop for LoopGuard<'_, P>
where
    P: ?Sized + Policy,
{
    /// Calls [`when_ended_early`] on the wrapped [`Policy`] reference
    /// with its internal state object.
    ///
    /// [`Policy`]: crate::iteration::Policy
    /// [`when_ended_early`]: crate::iteration::Policy::when_ended_early
    fn drop(&mut self) {
        self.policy.when_ended_early(&mut self.state);
    }
}

/// An unlimited iteration policy.
///
/// The [`Policy`] implementation for a value of this token type bypasses
/// any checking on iterations of a polling loop, allowing it to continue
/// indefinitely until ended by logic in the loop body (normally when
/// an asynchronous source is pending or the poll resolves with an output
/// value).
#[derive(Debug)]
pub struct Unlimited {}

impl Policy for Unlimited {
    type State = ();

    #[inline]
    fn begin(&mut self) {}

    #[inline]
    fn yield_check(&self, _: &mut ()) -> bool {
        false
    }
}

/// An iteration policy with a limit on the number of consecutive iterations.
///
/// The [`Policy`] implementation of this type runs a counter on the number
/// of iterations and, when the given limit is reached, instructs the
/// [`yield_check`](Policy::yield_check) caller to yield.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Limit(NonZeroU32);

impl Limit {
    /// Creates a policy checker instance with the given limit.
    #[inline]
    pub const fn new(value: NonZeroU32) -> Self {
        Limit(value)
    }
}

impl Policy for Limit {
    type State = u32;

    #[inline]
    fn begin(&mut self) -> u32 {
        self.0.into()
    }

    #[inline]
    fn yield_check(&self, counter: &mut u32) -> bool {
        *counter -= 1;
        *counter == 0
    }
}
