/// Ye Olde Await Macro
///
/// Basically a translation of polling to yielding. This crate's macro is
/// reexported in the `futures_await` crate, you should not use this crate
/// specifically. If I knew how to define this macro in the `futures_await`
/// crate I would. Ideally this crate would not exist.

// TODO: how to define this in the `futures_await` crate but have it still
// importable via `futurses_await::prelude::await`?

#[macro_export]
macro_rules! await {
    ($e:expr) => ({
        let mut future = $e;
        loop {
            match ::futures::Future::poll(&mut future) {
                ::futures::__rt::Ok(::futures::Async::Ready(e)) => {
                    break ::futures::__rt::Ok(e)
                }
                ::futures::__rt::Ok(::futures::Async::NotReady) => {}
                ::futures::__rt::Err(e) => {
                    break ::futures::__rt::Err(e)
                }
            }
            yield ::futures::Async::NotReady
        }
    })
}

// TODO: This macro needs to use an extra temporary variable because of
// rust-lang/rust#44197, once that's fixed this should just use $e directly
// inside the yield expression
#[macro_export]
macro_rules! stream_yield {
    ($e:expr) => ({
        let e = $e;
        yield ::futures::Async::Ready(e)
    })
}
