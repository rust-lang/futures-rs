extern crate futures;

use futures::*;

#[test]
fn fuse() {
    let mut task = Task::new();
    let mut future = finished::<i32, u32>(2).fuse();
    assert!(future.poll(&mut task).is_ready());
    assert!(future.poll(&mut task).is_not_ready());
}

#[test]
#[should_panic]
fn fuse_panic() {
    let mut task = Task::new();
    let mut future = finished::<i32, u32>(2).fuse();
    assert!(future.poll(&mut task).is_ready());
    assert!(future.poll(&mut task).is_ready());
}
