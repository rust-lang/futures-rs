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
///     done(a -> a) => a + 1,
///     done(b -> _) => 0,
/// };
/// assert_eq!(res, 5);
/// # });
/// ```
///
/// In addition to `future(...)` matchers, `select!` accepts an `complete`
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
///         done(a -> a) => total += a,
///         done(b -> b) => total += b,
///         complete => break,
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
        @codegen
        dones $done:tt
        nexts $next:tt
        default $default:tt
        complete ()
        used_labels $used_labels:tt
    ) => {
        $crate::select! {
            @codegen
            dones $done
            nexts $next
            default $default
            complete (
                panic!("all futures in select! were completed, \
                       but no `complete =>` handler was provided"))
            used_labels $used_labels
        }
    };
    // Remember the lack of a default
    (
        @codegen
        dones $done:tt
        nexts $next:tt
        default ()
        complete $complete:tt
        used_labels $used_labels:tt
    ) => {
        $crate::select! {
            @codegen
            dones $done
            nexts $next
            default ( unreachable!() )
            no_default true
            complete $complete
            used_labels $used_labels
        }
    };

    (
        @codegen
        dones ($( $fut_label:ident ($fut_name:ident -> $fut_pat:pat) => $fut_body:expr, )*)
        nexts ($( $st_label:ident ($st_name:ident -> $st_pat:pat) => $st_body:expr, )*)
        default ($default:expr)
        $( no_default $no_default:ident )*
        complete ($complete:expr)
        used_labels ($( $used_label:ident, )*)
    ) => { {

        // Require all arguments to be `Unpin` so that we don't have to pin them,
        // allowing uncompleted futures to be reused by the caller after the
        // `select!` resolves.
        //
        // Additionally, require all arguments to implement `FusedFuture` so that
        // we can ensure that futures aren't polled after completion by use
        // in successive `select!` statements.
        $(
            $crate::async_await::assert_unpin(&$fut_name);
            $crate::async_await::assert_fused_future(&$fut_name);
        )*
        $(
            $crate::async_await::assert_unpin(&$st_name);
            $crate::async_await::assert_fused_stream(&$st_name);
        )*

        #[allow(bad_style)]
        enum __PrivResult<$($used_label,)*> {
            $(
                $used_label($used_label),
            )*
            __Default,
			__Complete,
        }

        let __priv_res = await!($crate::future::poll_fn(|lw| {
            let mut __any_polled = false;

            $(
                let mut $fut_label = &mut $fut_name;
            )*
            $(
                let mut $st_label = $crate::stream::StreamExt::next(&mut $st_name);
            )*

            $(
                if !$crate::async_await::FusedFuture::is_terminated(& $used_label) {
                    __any_polled = true;
                    match $crate::core_reexport::future::Future::poll(
                        $crate::core_reexport::pin::Pin::new(&mut $used_label), lw)
                    {
                        $crate::core_reexport::task::Poll::Ready(x) =>
                            return $crate::core_reexport::task::Poll::Ready(
                                __PrivResult::$used_label(x)
                            ),
                        $crate::core_reexport::task::Poll::Pending => {},
                    }
                }
            )*

            if !__any_polled {
                $crate::core_reexport::task::Poll::Ready(
                    __PrivResult::__Complete)
            } else {
                $(
                    // only if there isn't a default case:
                    drop($no_default);
                    return $crate::core_reexport::task::Poll::Pending;
                )*
                #[allow(unreachable_code)]
                $crate::core_reexport::task::Poll::Ready(
                    __PrivResult::__Default)
            }
        }));
        match __priv_res {
            $(
                __PrivResult::$fut_label($fut_pat) => {
                    $fut_body
                }
            )*
            $(
                __PrivResult::$st_label($st_pat) => {
                    $st_body
                }
            )*
            __PrivResult::__Default => {
                $default
            }
			__PrivResult::__Complete => {
				$complete
			}
        }
    } };

    // All tokens have been successfully parsed into separate cases--
    // continue to individual case parsing and separation.
    (@parse_list
        cases($($head:tt)*)
        tokens()
    ) => {
        $crate::select!(
            @parse_case
            dones()
            nexts()
            default()
            complete()
            cases($($head)*)
            labels(
                case1
                case2
                case3
                case4
                case5
                case6
                case7
                case8
                case9
                case10
                case11
                case12
                case13
                case14
                case15
                case16
                case17
                case18
                case19
                case20
                case21
                case22
                case23
                case24
                case25
                case26
                case27
                case28
                case29
                case30
                case31
            )
            used_labels ()
        )
    };

    // insert empty argument list after `default` and `complete`
    (@parse_list
        cases($($head:tt)*)
        tokens(default => $($tail:tt)*)
    ) => {
        $crate::select!(
            @parse_list
            cases($($head)*)
            tokens(default() => $($tail)*)
        )
    };
    (@parse_list
        cases($($head:tt)*)
        tokens(complete => $($tail:tt)*)
    ) => {
        $crate::select!(
            @parse_list
            cases($($head)*)
            tokens(complete() => $($tail)*)
        )
    };

    // The first case is separated by a comma.
    (@parse_list
        cases($($head:tt)*)
        tokens($case:ident $args:tt => $body:expr, $($tail:tt)*)
    ) => {
        $crate::select!(
            @parse_list
            cases($($head)* $case $args => { $body },)
            tokens($($tail)*)
        )
    };
    // Print an error if there is a semicolon after the block.
    (@parse_list
        cases($($head:tt)*)
        tokens($case:ident $args:tt => $body:block; $($tail:tt)*)
    ) => {
        compile_error!("did you mean to put a comma instead of the semicolon after `}`?")
    };

    // Don't require a comma after the case if it has a proper block.
    (@parse_list
        cases($($head:tt)*)
        tokens($case:ident $args:tt => $body:block $($tail:tt)*)
    ) => {
        $crate::select!(
            @parse_list
            cases($($head)* $case $args => { $body },)
            tokens($($tail)*)
        )
    };
    // Only one case remains.
    (@parse_list
        cases($($head:tt)*)
        tokens($case:ident $args:tt => $body:expr)
    ) => {
        $crate::select!(
            @parse_list
            cases($($head)* $case $args => { $body },)
            tokens()
        )
    };
    // Accept a trailing comma at the end of the list.
    (@parse_list
        cases($($head:tt)*)
        tokens($case:ident $args:tt => $body:expr,)
    ) => {
        $crate::select!(
            @parse_list
            ($($head)* $case $args => { $body },)
            ()
        )
    };
    // Diagnose and print an error.
    (@parse_list
        cases($($head:tt)*)
        tokens($(tail:tt)*)
    ) => {
        $crate::select!(@parse_list_error1 $($tail)*)
    };
    // Stage 1: check the case type.
    (@parse_list_error1 recv $($tail:tt)*) => {
        $crate::select!(@parse_list_error2 done $($tail)*)
    };
    (@parse_list_error1 send $($tail:tt)*) => {
        $crate::select!(@parse_list_error2 next $($tail)*)
    };
    (@parse_list_error1 default $($tail:tt)*) => {
        $crate::select!(@parse_list_error2 default $($tail)*)
    };
    (@parse_list_error1 complete $($tail:tt)*) => {
        $crate::select!(@parse_list_error2 complete $($tail)*)
    };
    (@parse_list_error1 $t:tt $($tail:tt)*) => {
        compile_error!(concat!(
            "expected one of `done(...)`, `next(...)`, `default`, or `complete`, found `",
            stringify!($t),
            "`",
        ))
    };
    (@parse_list_error1 $($tail:tt)*) => {
        $crate::select!(@parse_list_error2 $($tail)*);
    };
    // Stage 2: check the argument list.
    (@parse_list_error2 $case:ident) => {
        compile_error!(concat!(
            "missing argument list after `",
            stringify!($case),
            "`",
        ))
    };
    (@parse_list_error2 $case:ident => $($tail:tt)*) => {
        compile_error!(concat!(
            "missing argument list after `",
            stringify!($case),
            "`",
        ))
    };
    (@parse_list_error2 $($tail:tt)*) => {
        $crate::select!(@parse_list_error3 $($tail)*)
    };
    // Stage 3: check the `=>` and what comes after it.
    (@parse_list_error3 $case:ident($($args:tt)*)) => {
        compile_error!(concat!(
            "missing `=>` after the argument list of `",
            stringify!($case),
            "`",
        ))
    };
    (@parse_list_error3 $case:ident($($args:tt)*) =>) => {
        compile_error!("expected expression after `=>`")
    };
    (@parse_list_error3 $case:ident($($args:tt)*) => $body:expr; $($tail:tt)*) => {
        compile_error!(concat!(
            "did you mean to put a comma instead of the semicolon after `",
            stringify!($body),
            "`?",
        ))
    };
    (@parse_list_error3 $case:ident($($args:tt)*) => recv($($a:tt)*) $($tail:tt)*) => {
        compile_error!("expected an expression after `=>`")
    };
    (@parse_list_error3 $case:ident($($args:tt)*) => send($($a:tt)*) $($tail:tt)*) => {
        compile_error!("expected an expression after `=>`")
    };
    (@parse_list_error3 $case:ident($($args:tt)*) => default($($a:tt)*) $($tail:tt)*) => {
        compile_error!("expected an expression after `=>`")
    };
    (@parse_list_error3 $case:ident($($args:tt)*) => $f:ident($($a:tt)*) $($tail:tt)*) => {
        compile_error!(concat!(
            "did you mean to put a comma after `",
            stringify!($f),
            "(",
            stringify!($($a)*),
            ")`?",
        ))
    };
    (@parse_list_error3 $case:ident($($args:tt)*) => $f:ident!($($a:tt)*) $($tail:tt)*) => {
        compile_error!(concat!(
            "did you mean to put a comma after `",
            stringify!($f),
            "!(",
            stringify!($($a)*),
            ")`?",
        ))
    };
    (@parse_list_error3 $case:ident($($args:tt)*) => $f:ident![$($a:tt)*] $($tail:tt)*) => {
        compile_error!(concat!(
            "did you mean to put a comma after `",
            stringify!($f),
            "![",
            stringify!($($a)*),
            "]`?",
        ))
    };
    (@parse_list_error3 $case:ident($($args:tt)*) => $f:ident!{$($a:tt)*} $($tail:tt)*) => {
        compile_error!(concat!(
            "did you mean to put a comma after `",
            stringify!($f),
            "!{",
            stringify!($($a)*),
            "}`?",
        ))
    };
    (@parse_list_error3 $case:ident($($args:tt)*) => $body:tt $($tail:tt)*) => {
        compile_error!(concat!(
            "did you mean to put a comma after `",
            stringify!($body),
            "`?",
        ))
    };
    (@parse_list_error3 $case:ident($($args:tt)*) $t:tt $($tail:tt)*) => {
        compile_error!(concat!(
            "expected `=>`, found `",
            stringify!($t),
            "`",
        ))
    };
    (@parse_list_error3 $case:ident $args:tt $($tail:tt)*) => {
        compile_error!(concat!(
            "expected an argument list, found `",
            stringify!($args),
            "`",
        ))
    };
    (@parse_list_error3 $($tail:tt)*) => {
        $crate::select!(@parse_list_error4 $($tail)*)
    };
    // Stage 4: fail with a generic error message.
    (@parse_list_error4 $($tail:tt)*) => {
        compile_error!("invalid syntax")
    };

    // Success! all cases were parsed
    (@parse_case
        dones $done:tt
        nexts $next:tt
        default $default:tt
        complete $complete:tt
        cases ()
        labels $labels:tt
        used_labels $used:tt
    ) => {
        $crate::select!(
            @codegen
            dones $done
            nexts $next
            default $default
            complete $complete
            used_labels $used
        )
    };
    // Error: no labels left
    (@parse_case
        dones $done:tt
        nexts $next:tt
        default $default:tt
        complete $complete:tt
        cases $cases:tt
        labels ()
        used_labels $used:tt
    ) => {
        compile_error!("too many cases in a `select!` block")
    };
    // Parse `done(future -> foo) => { ... }`
    (@parse_case
        dones ($($done:tt)*)
        nexts $next:tt
        default $default:tt
        complete $complete:tt
        cases (done($fut:ident -> $pat:pat) => $body:tt, $($tail:tt)*)
        labels ($label:tt $($labels:tt)*)
        used_labels ($($used:tt)*)
    ) => {
        $crate::select!(
            @parse_case
            dones ($($done)* $label ($fut -> $pat) => $body,)
            nexts $next
            default $default
            complete $complete
            cases ($($tail)*)
            labels ($($labels)*)
            used_labels ($($used)* $label,)
        )
    };
    (@parse_case
        dones $done:tt
        nexts $next:tt
        default $default:tt
        complete $complete:tt
        cases (done $t:tt => $body:tt, $($tail:tt)*)
        labels ($label:tt $($labels:tt)*)
        used_labels $used:tt
    ) => {
        compile_error!(concat!(
            "invalid syntax to `done`: `",
            stringify!($t),
            "`, expected `done(fut -> pat)`",
        ))
    };
    // Parse `next(stream -> foo) => { ... }`
    (@parse_case
        dones $done:tt
        nexts ($($next:tt)*)
        default $default:tt
        complete $complete:tt
        cases (next($stream:ident -> $pat:pat) => $body:tt, $($tail:tt)*)
        labels ($label:tt $($labels:tt)*)
        used_labels ($($used:tt)*)
    ) => {
        $crate::select!(
            @parse_case
            dones $done
            nexts ($($next)* $label ($stream -> $pat) => $body,)
            default $default
            complete $complete
            cases ($($tail)*)
            labels ($($labels)*)
            used_labels ($($used)* $label,)
        )
    };
    (@parse_case
        dones $done:tt
        nexts $next:tt
        default $default:tt
        complete $complete:tt
        cases (next $t:tt => $body:tt, $($tail:tt)*)
        labels ($label:tt $($labels:tt)*)
        used_labels $used:tt
    ) => {
        compile_error!(concat!(
            "invalid syntax to `next`: `",
            stringify!($t),
            "`, expected `next(stream -> pat)`",
        ))
    };
    // Parse `default => { ... }`
    (@parse_case
        dones $done:tt
        nexts $next:tt
        default ()
        complete $complete:tt
        cases (default() => $body:tt, $($tail:tt)*)
        labels $labels:tt
        used_labels $used:tt
    ) => {
        $crate::select!(
            @parse_case
            dones $done
            nexts $next
            default ($body)
            complete $complete
            cases ($($tail)*)
            labels $labels
            used_labels $used
        )
    };
    // Error on multiple `default`s
    (@parse_case
        dones $done:tt
        nexts $next:tt
        default $default:tt
        complete $complete:tt
        cases (default() => $body:tt, $($tail:tt)*)
        labels $labels:tt
        used_labels $used:tt
    ) => {
        compile_error!("there can only be one `default` case in a `select!` block")
    };
    // Parse `complete => { ... }`
    (@parse_case
        dones $done:tt
        nexts $next:tt
        default $default:tt
        complete ()
        cases (complete() => $body:tt, $($tail:tt)*)
        labels $labels:tt
        used_labels $used:tt
    ) => {
        $crate::select!(
            @parse_case
            dones $done
            nexts $next
            default $default
            complete ($body)
            cases ($($tail)*)
            labels $labels
            used_labels $used
        )
    };
    // Error on multiple `complete`s
    (@parse_case
        dones $done:tt
        nexts $next:tt
        default $default:tt
        complete $complete:tt
        cases (complete() => $body:tt, $($tail:tt)*)
        labels $labels:tt
        used_labels $used:tt
    ) => {
        compile_error!("there can only be one `complete` case in a `select!` block")
    };

    // Catch errors in case parsing
    (@parse_case
        dones $done:tt
        nexts $next:tt
        default $default:tt
        complete $complete:tt
        cases ($case:ident $args:tt => $body:tt, $($tail:tt)*)
        labels $labels:tt
        used_labels $used:tt
    ) => {
        compile_error!(concat!(
            "expected one of `done(fut -> x)`, `next(stream -> x)`, `default`, or `complete`, found `",
            stringify!($case), stringify!($args),
            "`",
        ))
    };

    // Catches a bug within this macro (should not happen).
    (@$($tokens:tt)*) => {
        compile_error!(concat!(
            "internal error in futures select macro: ",
            stringify!(@$($tokens)*),
        ))
    };

    ($($case:ident $(($($args:tt)*))* => $body:expr $(,)*)*) => {
        $crate::select!(
            @parse_list
            cases()
            tokens($($case $(($($args)*))* => $body,)*)
        )
    };

    ($($tokens:tt)*) => {
        $crate::select!(
            @parse_list
            cases()
            tokens($($tokens)*)
        )
    };
}
