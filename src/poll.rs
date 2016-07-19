
#[macro_export]
macro_rules! try_poll {
    ($e:expr) => (match $e {
        $crate::Poll::NotReady => return $crate::Poll::NotReady,
        $crate::Poll::Ok(t) => Ok(t),
        $crate::Poll::Err(e) => Err(e),
    })
}

/// Possible return values from the `Future::poll` method.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Poll<T, E> {
    /// Indicates that the future is not ready yet, ask again later.
    NotReady,

    /// Indicates that the future has completed successfully, and this value is
    /// what the future completed with.
    Ok(T),

    /// Indicates that the future has failed, and this error what the future
    /// failed with.
    Err(E),
}

impl<T, E> Poll<T, E> {
    /// Change the success type of this `Poll` value with the closure provided
    pub fn map<F, U>(self, f: F) -> Poll<U, E>
        where F: FnOnce(T) -> U
    {
        match self {
            Poll::NotReady => Poll::NotReady,
            Poll::Ok(t) => Poll::Ok(f(t)),
            Poll::Err(e) => Poll::Err(e),
        }
    }

    /// Change the error type of this `Poll` with the closure provided
    pub fn map_err<F, U>(self, f: F) -> Poll<T, U>
        where F: FnOnce(E) -> U
    {
        match self {
            Poll::NotReady => Poll::NotReady,
            Poll::Ok(t) => Poll::Ok(t),
            Poll::Err(e) => Poll::Err(f(e)),
        }
    }

    /// Returns whether this is `Poll::NotReady`
    pub fn is_not_ready(&self) -> bool {
        match *self {
            Poll::NotReady => true,
            _ => false,
        }
    }

    /// Returns whether this is either `Poll::Ok` or `Poll::Err`
    pub fn is_ready(&self) -> bool {
        !self.is_not_ready()
    }

    /// Unwraps this `Poll` into a `Result`, panicking if it's not ready.
    pub fn unwrap(self) -> Result<T, E> {
        match self {
            Poll::Ok(t) => Ok(t),
            Poll::Err(t) => Err(t),
            Poll::NotReady => panic!("unwrapping a Poll that wasn't ready"),
        }
    }
}

impl<T, E> From<Result<T, E>> for Poll<T, E> {
    fn from(r: Result<T, E>) -> Poll<T, E> {
        match r {
            Ok(t) => Poll::Ok(t),
            Err(t) => Poll::Err(t),
        }
    }
}
