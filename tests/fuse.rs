extern crate futures;

use futures::*;
use futures::task::Task;

#[test]
fn fuse() {
    let mut future = finished::<i32, u32>(2).fuse();
    Task::new().enter(|| {
        assert!(future.poll().is_ready());
        assert!(future.poll().is_not_ready());
    })
}
