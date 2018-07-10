/// Extracts the successful type of a `Poll<Result<T, E>>`.
///
/// This macro bakes in propagation of `Pending` and `Err` signals by returning early.
#[macro_export]
macro_rules! try_ready {
    ($x:expr) => {
        match $x {
            $crate::core_reexport::task::Poll::Ready(Ok(x)) => x,
            $crate::core_reexport::task::Poll::Ready(Err(e)) =>
                return $crate::core_reexport::task::Poll::Ready(Err(e.into())),
            $crate::core_reexport::task::Poll::Pending =>
                return $crate::core_reexport::task::Poll::Pending,
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
            $crate::core_reexport::task::Poll::Ready(Ok(x)) =>
                $crate::core_reexport::task::Poll::Ready(x),
            $crate::core_reexport::task::Poll::Ready(Err(e)) =>
                return $crate::core_reexport::task::Poll::Ready(Err(e.into())),
            $crate::core_reexport::task::Poll::Pending =>
                $crate::core_reexport::task::Poll::Pending,
        }
    }
}

/// Extracts the successful type of a `Poll<T>`.
///
/// This macro bakes in propagation of `Pending` signals by returning early.
#[macro_export]
macro_rules! ready {
    ($e:expr) => (match $e {
        $crate::core_reexport::task::Poll::Ready(t) => t,
        $crate::core_reexport::task::Poll::Pending =>
            return $crate::core_reexport::task::Poll::Pending,
    })
}
