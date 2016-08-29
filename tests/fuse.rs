extern crate futures;

use futures::*;

#[test]
fn fuse() {
    let mut future = finished::<i32, u32>(2).fuse();
    futures::task::ThreadTask::new().enter(|| {
        assert!(future.poll().is_ready());
        assert!(future.poll().is_not_ready());
    })
}
