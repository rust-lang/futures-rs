//! The `select` macro.

/// Polls multiple futures simultaneously, executing the branch for the future
/// that finishes first.
///
/// `select!` can select over futures with different output types, but each
/// branch has to have the same return type. Inside each branch, the respective
/// future's output is available via a variable with the same name as the future.
///
/// This macro is only usable inside of async functions, closures, and blocks.
///
/// # Examples
///
/// ```
/// #![feature(pin, async_await, await_macro, futures_api)]
/// # futures::executor::block_on(async {
/// use futures::{select, future};
/// let mut a = future::ready(4);
/// let mut b: future::Empty<()> = future::empty();
///
/// let res = select! {
///     a => a + 1,
///     b => 0,
/// };
/// assert_eq!(res, 5);
/// # });
/// ```
#[macro_export]
macro_rules! select {
    () => {
        compile_error!("The `select!` macro requires at least one branch")
    };
    ($(
        $name:ident => $body:expr,
    )*) => { {
        // Require all arguments to be `Unpin` so that we don't have to pin them,
        // allowing uncompleted futures to be reused by the caller after the
        // `select!` resolves.
        $(
            $crate::async_await::assert_unpin(&$name);
        )*

        #[allow(bad_style)]
        enum __PrivResult<$($name,)*> {
            $(
                $name($name),
            )*
        }

        let __priv_res = loop {
            $(
                let poll_res = $crate::poll!($crate::core_reexport::mem::PinMut::new(
                        &mut $name));
                if let $crate::core_reexport::task::Poll::Ready(x) = poll_res {
                    break __PrivResult::$name(x);
                }
            )*
            $crate::pending!();
        };
        match __priv_res {
            $(
                __PrivResult::$name($name) => {
                    let _ = $name; // Suppress "unused" warning for binding name
                    $body
                }
            )*
        }
    } };
}
