extern crate futures;

use std::mem;

use futures::sync::mpsc;
use futures::stream::select_all;

mod support;

#[test]
fn works_1() {
    let (a_tx, a_rx) = mpsc::unbounded::<u32>();
    let (b_tx, b_rx) = mpsc::unbounded::<u32>();
    let (c_tx, c_rx) = mpsc::unbounded::<u32>();

    let streams = vec![a_rx, b_rx, c_rx];

    let stream = select_all(streams);
    let mut spawn = futures::executor::spawn(stream);

    b_tx.unbounded_send(99).unwrap();
    a_tx.unbounded_send(33).unwrap();
    assert_eq!(Some(Ok(33)), spawn.wait_stream());
    assert_eq!(Some(Ok(99)), spawn.wait_stream());

    // make sure we really return in the order items become ready
    // so now try it the other way around as before
    assert!(!spawn.poll_stream_notify(&support::notify_noop(), 0).unwrap().is_ready());
    b_tx.unbounded_send(99).unwrap();
    a_tx.unbounded_send(33).unwrap();
    assert_eq!(Some(Ok(99)), spawn.wait_stream());
    assert_eq!(Some(Ok(33)), spawn.wait_stream());

    c_tx.unbounded_send(42).unwrap();
    assert_eq!(Some(Ok(42)), spawn.wait_stream());
    a_tx.unbounded_send(43).unwrap();
    assert_eq!(Some(Ok(43)), spawn.wait_stream());

    mem::drop((a_tx, b_tx, c_tx));
    assert_eq!(None, spawn.wait_stream());
}
