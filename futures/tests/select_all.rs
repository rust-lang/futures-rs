extern crate futures;

use futures::executor::block_on;
use futures::prelude::*;
use futures::future::{ok, select_all, err};

#[test]
fn smoke() {
    let v = vec![
        ok(1),
        err(2),
        ok(3),
    ];

    let (i, idx, v) = block_on(select_all(v)).ok().unwrap();
    assert_eq!(i, 1);
    assert_eq!(idx, 0);

    let (i, idx, v) = block_on(select_all(v)).err().unwrap();
    assert_eq!(i, 2);
    assert_eq!(idx, 0);

    let (i, idx, v) = block_on(select_all(v)).ok().unwrap();
    assert_eq!(i, 3);
    assert_eq!(idx, 0);

    assert!(v.is_empty());
}
