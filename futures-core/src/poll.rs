
pub use core::task::Poll;

/// A macro for extracting the successful type of a `Poll<Result<T, E>>`.
///
/// This macro bakes in propagation of `Pending` and `Err` signals by returning early.
#[macro_export]
macro_rules! try_ready {
    ($x:expr) => {
        match $x {
            $crate::Poll::Ready(Ok(x)) => x,
            $crate::Poll::Ready(Err(e)) => return $crate::Poll::Ready(Err(e.into())),
            $crate::Poll::Pending => return $crate::Poll::Pending,
        }
    }
}


/// A macro for extracting `Poll<T>` from `Poll<Result<T, E>>`.
///
/// This macro bakes in propagation of `Err` signals by returning early.
/// This macro bakes in propagation of `Pending` and `Err` signals by returning early.
#[macro_export]
macro_rules! try_poll {
    ($x:expr) => {
        match $x {
            $crate::Poll::Ready(Ok(x)) => $crate::Poll::Ready(x),
            $crate::Poll::Ready(Err(e)) => return $crate::Poll::Ready(Err(e.into())),
            $crate::Poll::Pending => $crate::Poll::Pending,
        }
    }
}

/// A macro for extracting the successful type of a `Poll<T>`.
///
/// This macro bakes in propagation of `Pending` signals by returning early.
#[macro_export]
macro_rules! ready {
    ($e:expr) => (match $e {
        $crate::Poll::Ready(t) => t,
        $crate::Poll::Pending => return $crate::Poll::Pending,
    })
}

pub trait PollExt<T> {
    /// Change the ready value of this `Poll` with the closure provided
    fn map<U, F>(self, f: F) -> Poll<U>
        where F: FnOnce(T) -> U;
    /// Returns whether this is `Poll::Ready`
    fn is_ready(&self) -> bool;
    /// Returns whether this is `Poll::Pending`
    fn is_pending(&self) -> bool {
        !self.is_ready()
    }
}

impl<T> PollExt<T> for Poll<T> {

    fn map<U, F>(self, f: F) -> Poll<U>
        where F: FnOnce(T) -> U
    {
        match self {
            Poll::Ready(t) => Poll::Ready(f(t)),
            Poll::Pending => Poll::Pending,
        }
    }

    fn is_ready(&self) -> bool {
        match *self {
            Poll::Ready(_) => true,
            Poll::Pending => false,
        }
    }
}

pub trait PollResultExt<T, E> {
    /// Change the success value of this `Poll` with the closure provided
    fn map_ok<U, F>(self, f: F) -> Poll<Result<U, E>>
        where F: FnOnce(T) -> U;
    /// Change the error value of this `Poll` with the closure provided
    fn map_err<U, F>(self, f: F) -> Poll<Result<T, U>>
        where F: FnOnce(E) -> U;
}

impl<T, E> PollResultExt<T, E> for Poll<Result<T, E>> {
    
    fn map_ok<U, F>(self, f: F) -> Poll<Result<U, E>>
        where F: FnOnce(T) -> U
    {
        match self {
            Poll::Ready(Ok(t)) => Poll::Ready(Ok(f(t))),
            Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
            Poll::Pending => Poll::Pending,
        }
    }

    fn map_err<U, F>(self, f: F) -> Poll<Result<T, U>>
        where F: FnOnce(E) -> U
    {
        match self {
            Poll::Ready(Ok(t)) => Poll::Ready(Ok(t)),
            Poll::Ready(Err(e)) => Poll::Ready(Err(f(e))),
            Poll::Pending => Poll::Pending,
        }
    }
}


