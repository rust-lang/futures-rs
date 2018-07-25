//! The `join` macro.

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
