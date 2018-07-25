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
            let mut $name = $crate::future::maybe_done(&mut $name);
            let mut $name = $crate::core_reexport::mem::PinMut::new(&mut $name);
        )*
        loop {
            $(
                if let $crate::core_reexport::task::Poll::Ready(()) =
                    $crate::poll!($name.reborrow())
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
            else if let Some($name) = $name.take_output() {
                let _ = $name; // suppress "unused" warning for binding name
                $body
            }
        )*
        else {
            unreachable!()
        }
    } };
}
