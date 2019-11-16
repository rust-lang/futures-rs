/// Extracts the successful type of a `Poll<T>`.
///
/// This macro bakes in propagation of `Pending` signals by returning early.
#[macro_export]
macro_rules! ready {
    ($e:expr $(,)?) => (match $e {
        $crate::core_reexport::task::Poll::Ready(t) => t,
        $crate::core_reexport::task::Poll::Pending =>
            return $crate::core_reexport::task::Poll::Pending,
    })
}

/// An eager polling loop with a customizable policy on iterations.
///
/// This macro helps implement eager polling loops in a way that prevents
/// uncooperative polling behavior. It's typical for such a loop to occur in
/// a [`Future`](core::future::Future) implementation that repeatedly polls
/// asynchronous event sources, most often a [`Stream`](crate::Stream), to
/// perform some internal work without resolving the future and resume polling
/// in the next loop iteration. If the source polls return `Ready` for many
/// consecutive iterations, such a loop will not break for the whole duration,
/// potentially starving other asynchronous operations in the same task
/// from being polled.
///
/// To prevent this, `poll_loop!` uses a [`Policy`](crate::iteration::Policy)
/// guard to check at the beginning of each iteration whether to yield
/// to the task by returning `Pending`. The first parameter of the macro
/// receives a mutable reference to the iteration policy checker.
/// The second parameter receives the reference to the
/// [`Context`](core::task::Context) passed to the poll function.
/// The third parameter is given the body of a loop iteration.
#[macro_export]
macro_rules! poll_loop {
    {$policy:expr, $cx:expr, $body:expr} => {
        #[allow(clippy::deref_addrof)]
        {
            let mut guard = $crate::iteration::Policy::begin(&mut *$policy);
            let _ = loop {
                if $crate::iteration::Policy::yield_check(&*$policy, &mut guard) {
                    break $crate::task::ToYield;
                }
                $body
            };

            $crate::core_reexport::task::Context::waker($cx).wake_by_ref();
            return $crate::core_reexport::task::Poll::Pending;
        }
    }
}

/// A token type used inside the [`poll_loop!`] macro.
///
/// `ToYield` can be used as a value of a `break` expression within an
/// iteration body given to the `poll_loop!` macro to break out of the loop.
/// The effect is to yield to the polling task, hence `break ToYield`.
#[derive(Debug)]
pub struct ToYield;
