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

/// An eager polling loop with a limit on repetitions.
///
/// This macro helps implement eager polling loops in a way that prevents
/// uncooperative polling behavior. It's typical for such a loop to occur
/// a [`Future`](core::future::Future) implementation that repeatedly polls
/// asynchronous event sources, most often a [`Stream`](crate::Stream), to
/// perform some internal work without resolving the future and resume polling
/// in the next loop iteration. If the source polls return `Ready` for many
/// consecutive iterations, such a loop will not break for the whole duration,
/// potentially starving other asynchronous operations in the same task
/// from being polled.
///
/// To prevent this, `poll_loop!` runs a counter on the number of iterations
/// to perform before yielding to the task by returning `Pending`, initialized
/// from the first parameter of the macro. The second parameter receives the
/// reference to the [`Context`](core::task::Context) passed to the poll
/// function, and the third parameter is given the body of a loop iteration.
#[macro_export]
macro_rules! poll_loop {
    {$yield_after:expr, $cx:expr, $body:expr} => {
        {
            let mut range = 0u32..$crate::core_reexport::convert::Into::into($yield_after);
            debug_assert!(
                range.end != 0,
                "0 used as the iteration limit in a poll loop",
            );
            let _ = loop {
                if range.next().is_none() {
                    break $crate::task::ToYield;
                }
                $body
            };

            #[cold]
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
