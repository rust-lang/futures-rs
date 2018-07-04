#![feature(pin, futures_api)]

extern crate futures;

use futures::prelude::*;
use futures::future;

mod support;

#[test]
fn fuse() {
    let mut future = future::ready::<i32>(2).fuse();
    support::panic_waker_cx(|cx| {
        assert!(future.poll_unpin(cx).is_ready());
        assert!(future.poll_unpin(cx).is_pending());
    })
}
