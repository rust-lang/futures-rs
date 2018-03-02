/// A macro for extracting the successful type of a `Poll<T, E>`.
///
/// This macro bakes in propagation of *both* errors and `Pending` signals by
/// returning early.
#[macro_export]
macro_rules! try_ready {
    ($e:expr) => (match $e {
        Ok($crate::Async::Ready(t)) => t,
        Ok($crate::Async::Pending) => return Ok($crate::Async::Pending),
        Err(e) => return Err(From::from(e)),
    })
}

/// A convenience wrapper for `Result<Async<T>, E>`.
///
/// `Poll` is the return type of the `poll` method on the `Future` trait.
///
/// * `Ok(Async::Ready(t))` means the future has successfully resolved.
/// * `Ok(Async::Pending)` means the future is not able to fully resolve yet.
///    The current task will be awoken when the future can make further
///    progress.
/// * `Err(e)` means that an error was encountered when attempting to complete
///    the future. `Future`s which have returned errors are complete, and
///    should not be polled again. However,`Stream`s that have returned errors
///    may not be complete and should still be polled.
pub type Poll<T, E> = Result<Async<T>, E>;

/// Indicates whether a value is available, or if the current task has been
/// scheduled for later wake-up instead.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Async<T> {
    /// Represents that a value is immediately ready.
    Ready(T),

    /// Represents that a value is not ready yet.
    ///
    /// When a function returns `Pending`, the function *must* also
    /// ensure that the current task is scheduled to be awoken when
    /// progress can be made.
    Pending,
}

impl<T> Async<T> {
    /// Change the success value of this `Async` with the closure provided
    pub fn map<U, F>(self, f: F) -> Async<U>
        where F: FnOnce(T) -> U
    {
        match self {
            Async::Ready(t) => Async::Ready(f(t)),
            Async::Pending => Async::Pending,
        }
    }

    /// Returns whether this is `Async::Ready`
    pub fn is_ready(&self) -> bool {
        match *self {
            Async::Ready(_) => true,
            Async::Pending => false,
        }
    }

    /// Returns whether this is `Async::Pending`
    pub fn is_pending(&self) -> bool {
        !self.is_ready()
    }
}

impl<T> From<T> for Async<T> {
    fn from(t: T) -> Async<T> {
        Async::Ready(t)
    }
}
