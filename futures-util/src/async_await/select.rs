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
        $(
            $crate::async_await::assert_unpin(&$name);
        )*
        #[allow(bad_style)]
        struct __priv<$($name,)*> {
            $(
                $name: $name,
            )*
        }
        let mut __priv_futures = __priv {
            $(
                $name: $crate::future::maybe_done(&mut $name),
            )*
        };
        loop {
            $(
                if let $crate::core_reexport::task::Poll::Ready(()) =
                    $crate::poll!($crate::core_reexport::mem::PinMut::new(
                        &mut __priv_futures.$name))
                {
                    break;
                }
            )*
            $crate::pending!();
        }
        if false {
            unreachable!()
        }
        $(
            else if let Some($name) =
                $crate::core_reexport::mem::PinMut::new(&mut __priv_futures.$name).take_output()
            {
                let _ = $name; // suppress "unused" warning for binding name
                $body
            }
        )*
        else {
            unreachable!()
        }
    } };
}
