use futures::executor::block_on;
use futures::future::{ready, select_all};

#[test]
fn smoke() {
    let v = vec![
        ready(1),
        ready(2),
        ready(3),
    ];

    let (i, idx, v) = block_on(select_all(v));
    assert_eq!(i, 1);
    assert_eq!(idx, 0);

    let (i, idx, v) = block_on(select_all(v));
    assert_eq!(i, 2);
    assert_eq!(idx, 0);

    let (i, idx, v) = block_on(select_all(v));
    assert_eq!(i, 3);
    assert_eq!(idx, 0);

    assert!(v.is_empty());
}
