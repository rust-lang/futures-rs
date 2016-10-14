extern crate futures;

use futures::*;

#[test]
fn ignore_failed() {
    let v = vec![
        failed(1).boxed(),
        failed(2).boxed(),
        finished(3).boxed(),
        finished(4).boxed(),
    ];

    let (i, v) = select_any(v).wait().ok().unwrap();
    assert_eq!(i, 3);

    assert!(v.len() == 1);

    let (i, v) = select_any(v).wait().ok().unwrap();
    assert_eq!(i, 4);

    assert!(v.len() == 0);
}

#[test]
fn last_failed() {
    let v = vec![
        finished(1).boxed(),
        failed(2).boxed(),
        failed(3).boxed(),
    ];

    let (i, v) = select_any(v).wait().ok().unwrap();
    assert_eq!(i, 1);

    assert!(v.len() == 2);

    let i = select_any(v).wait().err().unwrap();
    assert_eq!(i, 3);
}
