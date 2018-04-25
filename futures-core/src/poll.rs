/// Indicates whether a value is available, or if the current task has been
/// scheduled for later wake-up instead.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Poll<T> {
    /// Represents that a value is immediately ready.
    Ready(T),

    /// Represents that a value is not ready yet.
    ///
    /// When a function returns `Pending`, the function *must* also
    /// ensure that the current task is scheduled to be awoken when
    /// progress can be made.
    Pending,
}

impl<T> Poll<T> {
    /// Change the success value of this `Poll` with the closure provided
    pub fn map<U, F>(self, f: F) -> Poll<U>
        where F: FnOnce(T) -> U
    {
        match self {
            Poll::Ready(t) => Poll::Ready(f(t)),
            Poll::Pending => Poll::Pending,
        }
    }

    /// Returns whether this is `Poll::Ready`
    pub fn is_ready(&self) -> bool {
        match *self {
            Poll::Ready(_) => true,
            Poll::Pending => false,
        }
    }

    /// Returns whether this is `Poll::Pending`
    pub fn is_pending(&self) -> bool {
        !self.is_ready()
    }
}

impl<T, E> Poll<Result<T, E>> {
    /// Convenience for with with a `PollResult` as a `Result`
    pub fn ok(self) -> Result<Poll<T>, E> {
        match self {
            Poll::Pending => Ok(Poll::Pending),
            Poll::Ready(Ok(t)) => Ok(Poll::Ready(t)),
            Poll::Ready(Err(e)) => Err(e),
        }
    }
}

impl<T> From<T> for Poll<T> {
    fn from(t: T) -> Poll<T> {
        Poll::Ready(t)
    }
}

/// Shorthand for a `Poll<Result<_, _>>` value.
pub type PollResult<T, E> = Poll<Result<T, E>>;

/// A macro for extracting the successful type of a `PollResult<T, E>`.
///
/// This macro bakes in propagation of *both* errors and `Pending` signals by
/// returning early.
#[macro_export]
macro_rules! try_ready {
    ($e:expr) => (match $e {
        $crate::Poll::Pending => return $crate::Poll::Pending,
        $crate::Poll::Ready(Ok(t)) => t,
        $crate::Poll::Ready(Err(e)) => return $crate::Poll::Ready(Err(From::from(e))),
    })
}

/// A macro for extracting the successful type of a `PollResult<T, E>`.
///
/// This macro bakes in propagation of errors, but not `Pending` signals, by
/// returning early.
#[macro_export]
macro_rules! try_poll {
    ($e:expr) => (match $e {
        $crate::Poll::Pending => $crate::Poll::Pending,
        $crate::Poll::Ready(Ok(t)) => $crate::Poll::Ready(t),
        $crate::Poll::Ready(Err(e)) => return $crate::Poll::Ready(Err(From::from(e))),
    })
}
