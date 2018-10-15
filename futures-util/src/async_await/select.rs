//! The `select` macro.

/// Polls multiple futures simultaneously, executing the branch for the future
/// that finishes first.
///
/// `select!` can select over futures with different output types, but each
/// branch has to have the same return type.
///
/// This macro is only usable inside of async functions, closures, and blocks.
///
/// # Examples
///
/// ```
/// #![feature(pin, async_await, await_macro, futures_api)]
/// # futures::executor::block_on(async {
/// use futures::future::{self, FutureExt};
/// use futures::select;
/// let mut a = future::ready(4);
/// let mut b = future::empty::<()>();
///
/// let res = select! {
///     future(a as a) => a + 1,
///     future(b as _) => 0,
/// };
/// assert_eq!(res, 5);
/// # });
/// ```
///
/// In addition to `future(...)` matchers, `select!` accepts an `all complete`
/// branch which can be used to match on the case where all the futures passed
/// to select have already completed.
///
/// ```
/// #![feature(pin, async_await, await_macro, futures_api)]
/// # futures::executor::block_on(async {
/// use futures::future::{self, FutureExt};
/// use futures::select;
/// let mut a = future::ready(4);
/// let mut b = future::ready(6);
/// let mut total = 0;
///
/// loop {
///     select! {
///         future(a as a) => total += a,
///         future(b as b) => total += b,
///         all complete => break,
///     };
/// }
/// assert_eq!(total, 10);
/// # });
/// ```
#[macro_export]
macro_rules! select {
    () => {
        compile_error!("The `select!` macro requires at least one branch")
    };
    (
        $(
            future($future_name:ident as $bound_pat:pat) => $body:expr,
        )*
        all complete => $complete:expr $(,)*
    ) => { {
        // Require all arguments to be `Unpin` so that we don't have to pin them,
        // allowing uncompleted futures to be reused by the caller after the
        // `select!` resolves.
        //
        // Additionally, require all arguments to implement `FusedFuture` so that
        // we can ensure that futures aren't polled after completion by use
        // in successive `select!` statements.
        $(
            $crate::async_await::assert_unpin(&$future_name);
            $crate::async_await::assert_fused_future(&$future_name);
        )*

        #[allow(bad_style)]
        enum __PrivResult<$($future_name,)*> {
            $(
                $future_name($future_name),
            )*
			__Complete,
        }

        let __priv_res = await!($crate::future::poll_fn(|lw| {
            let mut __any_polled = false;

            $(
                if !$crate::async_await::FusedFuture::is_terminated(& $future_name) {
                    __any_polled = true;
                    match $crate::core_reexport::future::Future::poll(
                        $crate::core_reexport::pin::Pin::new(&mut $future_name), lw)
                    {
                        $crate::core_reexport::task::Poll::Ready(x) =>
                            return $crate::core_reexport::task::Poll::Ready(
                                __PrivResult::$future_name(x)
                            ),
                        $crate::core_reexport::task::Poll::Pending => {},
                    }
                }
            )*

            if !__any_polled {
                return $crate::core_reexport::task::Poll::Ready(
                    __PrivResult::__Complete);
            }

            $crate::core_reexport::task::Poll::Pending
        }));
        match __priv_res {
            $(
                __PrivResult::$future_name($bound_pat) => {
                    $body
                }
            )*
			__PrivResult::__Complete => {
				$complete
			}
        }
    } };

	(
        $(
            future($future_name:ident as $bound_pat:pat) => $body:expr $(,)*
        )*
    ) => {
        $crate::select! {
            $(
                future($future_name as $bound_pat) => $body,
            )*
            all complete =>
                panic!("all futures in select! were completed, \
                       but no `all complete =>` handler was provided"),
        }
    };
}
