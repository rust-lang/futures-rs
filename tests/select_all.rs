extern crate futures;

use futures::prelude::*;
use futures::future::{blocking, ok, select_all, err};

#[test]
fn smoke() {
    let v = vec![
        ok(1),
        err(2),
        ok(3),
    ];

    let (i, idx, v) = blocking(select_all(v)).wait().ok().unwrap();
    assert_eq!(i, 1);
    assert_eq!(idx, 0);

    let (i, idx, v) = blocking(select_all(v)).wait().err().unwrap();
    assert_eq!(i, 2);
    assert_eq!(idx, 0);

    let (i, idx, v) = blocking(select_all(v)).wait().ok().unwrap();
    assert_eq!(i, 3);
    assert_eq!(idx, 0);

    assert!(v.is_empty());
}
