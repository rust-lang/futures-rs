//! The `join` macro.

/// Polls multiple futures simultaneously, returning a tuple
/// of all results once complete.
///
/// While `join!(a, b)` is similar to `(a.await, b.await)`,
/// `join!` polls both futures concurrently and therefore is more efficent.
///
/// This macro is only usable inside of async functions, closures, and blocks.
/// It is also gated behind the `async-await` feature of this library, which is
/// _not_ activated by default.
///
/// # Examples
///
/// ```
/// #![feature(async_await)]
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
            // Move future into a local so that it is pinned in one place and
            // is no longer accessible by the end user.
            let mut $fut = $crate::future::maybe_done($fut);
        )*
        $crate::future::poll_fn(move |cx| {
            let mut all_done = true;
            $(
                all_done &= $crate::core_reexport::future::Future::poll(
                    unsafe { $crate::core_reexport::pin::Pin::new_unchecked(&mut $fut) }, cx).is_ready();
            )*
            if all_done {
                $crate::core_reexport::task::Poll::Ready(($(
                    unsafe { $crate::core_reexport::pin::Pin::new_unchecked(&mut $fut) }.take_output().unwrap(),
                )*))
            } else {
                $crate::core_reexport::task::Poll::Pending
            }
        }).await
    } }
}

/// Polls multiple futures simultaneously, resolving to a [`Result`] containing
/// either a tuple of the successful outputs or an error.
///
/// `try_join!` is similar to [`join!`], but completes immediately if any of
/// the futures return an error.
///
/// This macro is only usable inside of async functions, closures, and blocks.
/// It is also gated behind the `async-await` feature of this library, which is
/// _not_ activated by default.
///
/// # Examples
///
/// When used on multiple futures that return `Ok`, `try_join!` will return
/// `Ok` of a tuple of the values:
///
/// ```
/// #![feature(async_await)]
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
/// #![feature(async_await)]
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
            // Move future into a local so that it is pinned in one place and
            // is no longer accessible by the end user.
            let mut $fut = $crate::future::maybe_done($fut);
        )*

        let res: $crate::core_reexport::result::Result<_, _> = $crate::future::poll_fn(move |cx| {
            let mut all_done = true;
            $(
                if $crate::core_reexport::future::Future::poll(
                    unsafe { $crate::core_reexport::pin::Pin::new_unchecked(&mut $fut) }, cx).is_pending()
                {
                    all_done = false;
                } else if unsafe { $crate::core_reexport::pin::Pin::new_unchecked(&mut $fut) }.output_mut().unwrap().is_err() {
                    // `.err().unwrap()` rather than `.unwrap_err()` so that we don't introduce
                    // a `T: Debug` bound.
                    return $crate::core_reexport::task::Poll::Ready(
                        $crate::core_reexport::result::Result::Err(
                            unsafe { $crate::core_reexport::pin::Pin::new_unchecked(&mut $fut) }.take_output().unwrap().err().unwrap()
                        )
                    );
                }
            )*
            if all_done {
                $crate::core_reexport::task::Poll::Ready(
                    $crate::core_reexport::result::Result::Ok(($(
                        // `.ok().unwrap()` rather than `.unwrap()` so that we don't introduce
                        // an `E: Debug` bound.
                        unsafe { $crate::core_reexport::pin::Pin::new_unchecked(&mut $fut) }.take_output().unwrap().ok().unwrap(),
                    )*))
                )
            } else {
                $crate::core_reexport::task::Poll::Pending
            }
        }).await;

        res
    } }
}
