extern crate futures;

use futures::future::{finished, Future};
use futures::executor;

mod support;
use support::*;

#[test]
fn fuse() {
    let mut future = executor::spawn(finished::<i32, u32>(2).fuse());
    assert!(future.poll_future(unpark_panic()).unwrap().is_ready());
    assert!(future.poll_future(unpark_panic()).unwrap().is_not_ready());
}
