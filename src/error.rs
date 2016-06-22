use std::any::Any;

/// The result yielded to the `Future::schedule` callback which indicates the
/// final result of a future.
///
/// Like `io::Result`, this is just a typedef around `Result<T, PollError<E>>`
/// and simply avoids writing lots all over the place.
pub type PollResult<T, E> = Result<T, PollError<E>>;

/// Possible errors that a future can be resolved with
pub enum PollError<E> {
    /// Generic payload indicating that a future has resolved with a custom
    /// error (e.g. an I/O error). This is the standard error that will likely
    /// come up the most.
    Other(E),

    /// Indicates that a future was canceled and all progress should halt
    /// immediately.
    Canceled,

    /// Indicates that this future somewhere along the way panicked and the
    /// payload was captured in a `Box<Any+Send>` here.
    Panicked(Box<Any + Send>),
}

impl<E> PollError<E> {
    /// Maps data contained in this error with the provided closure.
    ///
    /// Note that the closure is not guaranteed to be called as not all variants
    /// of a `PollError` have data to call it with.
    pub fn map<F: FnOnce(E) -> E2, E2>(self, f: F) -> PollError<E2> {
        match self {
            PollError::Panicked(e) => PollError::Panicked(e),
            PollError::Canceled => PollError::Canceled,
            PollError::Other(e) => PollError::Other(f(e)),
        }
    }
}
