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
        let ret;
        loop {
            match ::futures::Future::poll(&mut future) {
                ::futures::__rt::Ok(::futures::Async::Ready(e)) => {
                    ret = ::futures::__rt::Ok(e);
                    break
                }
                ::futures::__rt::Ok(::futures::Async::NotReady) => yield,
                ::futures::__rt::Err(e) => {
                    ret = ::futures::__rt::Err(e);
                    break
                }
            }
        }
        ret
    })
}
