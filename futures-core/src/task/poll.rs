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
/// To prevent this, `poll_loop!` uses the [`Policy`](crate::iteration::Policy)
/// protocol to check at the end of each iteration whether to yield
/// to the task by returning [`Pending`](core::task::poll::Poll::Pending).
/// The first parameter of the macro receives a mutable pointer to the
/// iteration policy checker implementing `Policy`.
/// The second parameter receives the reference to the
/// [`Context`](core::task::Context) passed to the poll function.
/// The body of a loop iteration is given in the third parameter.
///
/// Note that a `continue` expression in the loop body has the effect of
/// bypassing the iteration policy check, so it should be used with caution.
/// `break` can be used as in the body of an unlabeled `loop`, also for
/// producing the value of the macro invocation used as an expression.
/// The loop is immediately terminated by `break` with no hidden effects.
#[macro_export]
macro_rules! poll_loop {
    {$policy:expr, $cx:expr, $body:expr} => {
        #[allow(clippy::deref_addrof)]
        {
            let mut guard = $crate::iteration::Policy::begin(&mut *$policy);
            loop {
                { $body }
                if $crate::iteration::Policy::yield_check(&*$policy, &mut guard) {
                    $crate::core_reexport::task::Context::waker($cx).wake_by_ref();
                    return $crate::core_reexport::task::Poll::Pending;
                }
            }
        }
    }
}
