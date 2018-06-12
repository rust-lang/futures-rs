extern crate futures;

use futures::prelude::*;
use futures::future::ok;

mod support;

#[test]
fn fuse() {
    let mut future = ok::<i32, u32>(2).fuse();
    support::panic_waker_cx(|cx| {
        assert!(future.poll(cx).unwrap().is_ready());
        assert!(future.poll(cx).unwrap().is_pending());
    })
}
