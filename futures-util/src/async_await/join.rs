//! The `join` macro.

/// Polls multiple futures simultaneously, returning a tuple
/// of all results once complete. 
/// 
/// While `join!(a, b)` is similar to `(await!(a), await!(b))`,
/// `join!` polls both futures concurrently and therefore is more efficent.
/// 
/// This macro is only usable inside of async functions, closures, and blocks.
/// 
/// # Examples
/// 
/// ```
/// #![feature(pin, async_await, await_macro, futures_api)]
/// # futures::executor::block_on(async {
/// use futures::{join, future};
///
/// let a = future::ready(1);
/// let b = future::ready(2);
/// 
/// assert_eq!(join!(a, b), (1, 2));
/// # });
/// ```
#[macro_export]
macro_rules! join {
    ($($fut:ident),*) => { {
        $(
            let mut $fut = $crate::future::maybe_done($fut);
            $crate::pin_mut!($fut);
        )*
        loop {
            let mut all_done = true;
            $(
                if let $crate::core_reexport::task::Poll::Pending = $crate::poll!($fut.reborrow()) {
                    all_done = false;
                }
            )*
            if all_done {
                break;
            } else {
                $crate::pending!();
            }
        }

        ($(
            $fut.reborrow().take_output().unwrap(),
        )*)
    } }
}
