use futures::executor::block_on_stream;
use futures::channel::mpsc;
use futures::stream::select_all;
use std::mem;

mod support;

#[test]
fn works_1() {
    let (a_tx, a_rx) = mpsc::unbounded::<u32>();
    let (b_tx, b_rx) = mpsc::unbounded::<u32>();
    let (c_tx, c_rx) = mpsc::unbounded::<u32>();

    let streams = vec![a_rx, b_rx, c_rx];

    let mut stream = block_on_stream(select_all(streams));

    b_tx.unbounded_send(99).unwrap();
    a_tx.unbounded_send(33).unwrap();
    assert_eq!(Some(Ok(33)), stream.next());
    assert_eq!(Some(Ok(99)), stream.next());

    b_tx.unbounded_send(99).unwrap();
    a_tx.unbounded_send(33).unwrap();
    assert_eq!(Some(Ok(33)), stream.next());
    assert_eq!(Some(Ok(99)), stream.next());

    c_tx.unbounded_send(42).unwrap();
    assert_eq!(Some(Ok(42)), stream.next());
    a_tx.unbounded_send(43).unwrap();
    assert_eq!(Some(Ok(43)), stream.next());

    mem::drop((a_tx, b_tx, c_tx));
    assert_eq!(None, stream.next());
}
