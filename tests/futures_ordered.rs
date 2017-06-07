extern crate futures;

use futures::{Async, Future, Stream};
use futures::stream::futures_ordered;
use futures::sync::oneshot;

mod support;
use support::{f_err, f_ok};

#[test]
fn works_1() {
    let into_futs = vec![f_ok(1), f_ok(2)];
    assert_eq!(futures_ordered(into_futs).collect().wait(), Ok(vec![1, 2]));

    let into_futs = vec![f_ok(1), f_err(2), f_ok(3)];
    assert_eq!(futures_ordered(into_futs).collect().wait(), Err(2));
}

#[test]
fn ordered() {
    let (a_tx, a_rx) = oneshot::channel::<u32>();
    let (b_tx, b_rx) = oneshot::channel::<u32>();
    let stream = futures_ordered(vec![a_rx.boxed(), b_rx.boxed()]);

    let mut spawn = futures::executor::spawn(stream);
    for _ in 0..10 {
        assert!(spawn.poll_stream(support::unpark_noop()).unwrap().is_not_ready());
    }

    // This should not cause the value from b to be returned since a hasn't
    // been evaluated yet.
    b_tx.send(33).unwrap();
    for _ in 0..10 {
        assert!(spawn.poll_stream(support::unpark_noop()).unwrap().is_not_ready());
    }

    a_tx.send(66).unwrap();

    // value from a
    let next = spawn.poll_stream(support::unpark_noop()).unwrap();
    assert_eq!(next, Async::Ready(Some(66)));
    // value from b
    let next = spawn.poll_stream(support::unpark_noop()).unwrap();
    assert_eq!(next, Async::Ready(Some(33)));
}
