// Ye Olde Await Macro
//
// Basically a translation of polling to yielding
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
