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
                if let $crate::core_reexport::task::Poll::Ready(()) = poll!($name.reborrow()) {
                    break;
                }
            )*
            pending!();
        }
        if false {
            unreachable!()
        }
        $(
            else if let Some($name) = $name.take_output() {
                $body
            }
        )*
        else {
            unreachable!()
        }
    } };
}

async fn num() -> usize { 5 }

#[allow(unused)]
async fn test_select_compiles() -> usize {
    let a = num();
    let b = num();
    pin_mut!(a, b);
    select! {
        a => {
            let x = num();
            a + await!(x)
        },
        b => b + 4,
    }
}
