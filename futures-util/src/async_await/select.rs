//! The `select` macro.

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
