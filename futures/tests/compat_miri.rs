#![cfg(feature = "compat")]
use futures::compat::Future01CompatExt;
use futures::executor::block_on;
use futures::future::TryFutureExt;

#[test]
// from https://github.com/rust-lang/futures-rs/issues/2514
fn miri_compat_two_way() {
    let fut = async { Ok(()) as Result<(), ()> };
    let fut = Box::pin(fut);
    let fut = fut.compat(); // 03as01
    let fut = fut.compat(); // 01as03
    block_on(fut).unwrap();
}
