#![feature(pin, arbitrary_self_types, futures_api)]

use futures::future;
use futures::prelude::*;

mod support;

#[test]
fn fuse() {
    let mut future = future::ready::<i32>(2).fuse();
    support::with_panic_waker_context(|cx| {
        assert!(future.poll_unpin(cx).is_ready());
        assert!(future.poll_unpin(cx).is_pending());
    })
}
