//! The `join` macro.

#[macro_export]
macro_rules! join {
    ($($fut:ident),*) => { {
        $(
            let mut $fut = $crate::future::maybe_done($fut);
            pin_mut!($fut);
        )*
        loop {
            let mut all_done = true;
            $(
                if let $crate::core_reexport::task::Poll::Pending = poll!($fut.reborrow()) {
                    all_done = false;
                }
            )*
            if all_done {
                break;
            } else {
                pending!();
            }
        }

        ($(
            $fut.reborrow().take_output().unwrap(),
        )*)
    } }
}

async fn a() {}
async fn b() -> usize { 5 }

#[allow(unused)]
async fn test_join_compiles() -> ((), usize) {
    let a = a();
    let b = b();
    join!(a, b)
}
