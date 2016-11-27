#[macro_use]
extern crate futures;

use futures::sync::oneshot;
use futures::stream::futures_unordered;
use futures::Future;

mod support;

#[test]
fn works_1() {
    let (a_tx, a_rx) = oneshot::channel::<u32>();
    let (b_tx, b_rx) = oneshot::channel::<u32>();
    let (c_tx, c_rx) = oneshot::channel::<u32>();

    let stream = futures_unordered(vec![a_rx, b_rx, c_rx]);

    let mut spawn = futures::executor::spawn(stream);
    b_tx.complete(99);
    assert_eq!(Some(Ok(99)), spawn.wait_stream());

    a_tx.complete(33);
    c_tx.complete(33);
    assert_eq!(Some(Ok(33)), spawn.wait_stream());
    assert_eq!(Some(Ok(33)), spawn.wait_stream());
    assert_eq!(None, spawn.wait_stream());
}


#[test]
fn works_2() {
    let (a_tx, a_rx) = oneshot::channel::<u32>();
    let (b_tx, b_rx) = oneshot::channel::<u32>();
    let (c_tx, c_rx) = oneshot::channel::<u32>();

    let stream = futures_unordered(vec![a_rx.boxed(), b_rx.join(c_rx).map(|(a, b)| a + b).boxed()]);

    let mut spawn = futures::executor::spawn(stream);
    a_tx.complete(33);
    b_tx.complete(33);
    assert!(spawn.poll_stream(support::unpark_noop()).unwrap().is_ready());
    c_tx.complete(33);
    assert!(spawn.poll_stream(support::unpark_noop()).unwrap().is_ready());
}
