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
                if $crate::poll!($fut.reborrow()).is_pending() {
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

/// Polls multiple futures simultaneously, resolving to a [`Result`] containing
/// either a tuple of the successful outputs or an error.
///
/// `try_join!` is similar to [`join!`], but completes immediately if any of
/// the futures return an error.
///
/// This macro is only usable inside of async functions, closures, and blocks.
///
/// # Examples
///
/// When used on multiple futures that return `Ok`, `try_join!` will return
/// `Ok` of a tuple of the values:
///
/// ```
/// #![feature(pin, async_await, await_macro, futures_api)]
/// # futures::executor::block_on(async {
/// use futures::{try_join, future};
///
/// let a = future::ready(Ok::<i32, i32>(1));
/// let b = future::ready(Ok::<u64, i32>(2));
///
/// assert_eq!(try_join!(a, b), Ok((1, 2)));
/// # });
/// ```
///
/// If one of the futures resolves to an error, `try_join!` will return
/// that error:
///
/// ```
/// #![feature(pin, async_await, await_macro, futures_api)]
/// # futures::executor::block_on(async {
/// use futures::{try_join, future};
///
/// let a = future::ready(Ok::<i32, i32>(1));
/// let b = future::ready(Err::<u64, i32>(2));
///
/// assert_eq!(try_join!(a, b), Err(2));
/// # });
/// ```
#[macro_export]
macro_rules! try_join {
    ($($fut:ident),*) => { {
        $(
            let mut $fut = $crate::future::maybe_done($fut);
            $crate::pin_mut!($fut);
        )*
        let res: $crate::core_reexport::result::Result<(), _> = loop {
            let mut all_done = true;
            $(
                if $crate::poll!($fut.reborrow()).is_pending() {
                    all_done = false;
                } else if $fut.reborrow().output_mut().unwrap().is_err() {
                    // `.err().unwrap()` rather than `.unwrap_err()` so that we don't introduce
                    // a `T: Debug` bound.
                    break $crate::core_reexport::result::Result::Err(
                        $fut.reborrow().take_output().unwrap().err().unwrap()
                    );
                }
            )*
            if all_done {
                break $crate::core_reexport::result::Result::Ok(());
            } else {
                $crate::pending!();
            }
        };

        res.map(|()| ($(
            // `.ok().unwrap()` rather than `.unwrap()` so that we don't introduce
            // an `E: Debug` bound.
            $fut.reborrow().take_output().unwrap().ok().unwrap(),
        )*))
    } }
}
