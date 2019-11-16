//! Helpers for cooperative polling behavior.

use core::num::NonZeroU32;

pub trait Policy {
    type Guard;

    fn begin(&mut self) -> Self::Guard;

    fn yield_check(&self, guard: &mut Self::Guard) -> bool;
}

#[derive(Debug)]
pub struct Unlimited {}

impl Policy for Unlimited {
    type Guard = ();

    #[inline]
    fn begin(&mut self) {}

    #[inline]
    fn yield_check(&self, _: &mut ()) -> bool {
        false
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Limit(NonZeroU32);

impl Limit {
    #[inline]
    pub const fn new(value: NonZeroU32) -> Self {
        Limit(value)
    }
}

impl Policy for Limit {
    type Guard = u32;

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

#[cfg(test)]
mod tests {
    use super::*;

    struct StatefulPolicy {
        limit: Option<u32>,
    }

    impl Policy for StatefulPolicy {
        type Guard = u32;

        fn begin(&mut self) -> u32 {
            0
        }

        fn yield_check(&self, guard: &mut u32) -> bool {
            *guard = guard.wrapping_add(1);
            match self.limit {
                None => false,
                Some(limit) => *guard >= limit,
            }
        }
    }
}
