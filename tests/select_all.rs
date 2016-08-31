extern crate futures;

use futures::*;

mod support;
use support::*;

#[test]
fn smoke() {
    let v = vec![
        finished(1).boxed(),
        failed(2).boxed(),
        finished(3).boxed(),
    ];

    let (i, idx, v) = task::spawn(select_all(v)).poll_future(unpark_panic())
                                                .unwrap().ok().unwrap();
    assert_eq!(i, 1);
    assert_eq!(idx, 0);

    let (i, idx, v) = task::spawn(select_all(v)).poll_future(unpark_panic())
                                                .unwrap().err().unwrap();
    assert_eq!(i, 2);
    assert_eq!(idx, 0);

    let (i, idx, v) = task::spawn(select_all(v)).poll_future(unpark_panic())
                                                .unwrap().ok().unwrap();
    assert_eq!(i, 3);
    assert_eq!(idx, 0);

    assert!(v.len() == 0);
}
