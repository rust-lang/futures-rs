//! Definition and trait implementations for the `Never` type,
//! a stand-in for the `!` type until it becomes stable.

use {Future, Stream, Poll};
use task;

/// A type with no possible values.
///
/// This is used to indicate values which can never be created, such as the
/// error type of infallible futures.
///
/// This type is a stable equivalent to the `!` type from `std`.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub enum Never {}

impl Never {
    /// Convert the `Never` type into any other type.
    pub fn never_into<T>(self) -> T {
        match self {}
    }
}

impl Future for Never {
    type Item = Never;
    type Error = Never;

    fn poll(&mut self, _: &mut task::Context) -> Poll<Never, Never> {
        match *self {}
    }
}

impl Stream for Never {
    type Item = Never;
    type Error = Never;

    fn poll_next(&mut self, _: &mut task::Context) -> Poll<Option<Never>, Never> {
        match *self {}
    }
}
