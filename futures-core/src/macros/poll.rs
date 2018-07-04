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
