#![feature(arbitrary_self_types, futures_api)]

use futures::future::{self, FutureExt};
use futures_test::task::panic_local_waker;

#[test]
fn fuse() {
    let mut future = future::ready::<i32>(2).fuse();
    let lw = &mut panic_local_waker();
    assert!(future.poll_unpin(lw).is_ready());
    assert!(future.poll_unpin(lw).is_pending());
}
