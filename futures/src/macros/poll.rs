/// Extracts the successful type of a `Poll<Result<T, E>>`.
///
/// This macro bakes in propagation of `Pending` and `Err` signals by returning early.
#[macro_export]
macro_rules! try_ready {
    ($x:expr) => {
        match $x {
            $crate::task::Poll::Ready(Ok(x)) => x,
            $crate::task::Poll::Ready(Err(e)) => return $crate::task::Poll::Ready(Err(e.into())),
            $crate::task::Poll::Pending => return $crate::task::Poll::Pending,
        }
    }
}


/// Extracts `Poll<T>` from `Poll<Result<T, E>>`.
///
/// This macro bakes in propagation of `Err` signals by returning early.
/// This macro bakes in propagation of `Pending` and `Err` signals by returning early.
#[macro_export]
macro_rules! try_poll {
    ($x:expr) => {
        match $x {
            $crate::task::Poll::Ready(Ok(x)) => $crate::task::Poll::Ready(x),
            $crate::task::Poll::Ready(Err(e)) => return $crate::task::Poll::Ready(Err(e.into())),
            $crate::task::Poll::Pending => $crate::task::Poll::Pending,
        }
    }
}

/// Extracts the successful type of a `Poll<T>`.
///
/// This macro bakes in propagation of `Pending` signals by returning early.
#[macro_export]
macro_rules! ready {
    ($e:expr) => (match $e {
        $crate::task::Poll::Ready(t) => t,
        $crate::task::Poll::Pending => return $crate::task::Poll::Pending,
    })
}
