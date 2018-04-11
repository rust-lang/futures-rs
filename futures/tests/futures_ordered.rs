extern crate futures;

use std::any::Any;

use futures::executor::{block_on, block_on_stream};
use futures::channel::oneshot;
use futures::stream::futures_ordered;
use futures::prelude::*;

mod support;

#[test]
fn works_1() {
    let (a_tx, a_rx) = oneshot::channel::<u32>();
    let (b_tx, b_rx) = oneshot::channel::<u32>();
    let (c_tx, c_rx) = oneshot::channel::<u32>();

    let mut stream = futures_ordered(vec![a_rx, b_rx, c_rx]);

    b_tx.send(99).unwrap();
    support::noop_waker_cx(|cx| {
        assert!(stream.poll_next(cx).unwrap().is_pending());
    });

    a_tx.send(33).unwrap();
    c_tx.send(33).unwrap();

    let mut iter = block_on_stream(stream);
    assert_eq!(Some(Ok(33)), iter.next());
    assert_eq!(Some(Ok(99)), iter.next());
    assert_eq!(Some(Ok(33)), iter.next());
    assert_eq!(None, iter.next());
}

#[test]
fn works_2() {
    let (a_tx, a_rx) = oneshot::channel::<u32>();
    let (b_tx, b_rx) = oneshot::channel::<u32>();
    let (c_tx, c_rx) = oneshot::channel::<u32>();

    let mut stream = futures_ordered(vec![
        Box::new(a_rx) as Box<Future<Item = _, Error = _>>,
        Box::new(b_rx.join(c_rx).map(|(a, b)| a + b)) as _,
    ]);

    support::noop_waker_cx(|cx| {
        a_tx.send(33).unwrap();
        b_tx.send(33).unwrap();
        assert!(stream.poll_next(cx).unwrap().is_ready());
        assert!(stream.poll_next(cx).unwrap().is_pending());
        c_tx.send(33).unwrap();
        assert!(stream.poll_next(cx).unwrap().is_ready());
    })
}

#[test]
fn from_iterator() {
    use futures::future::ok;
    use futures::stream::FuturesOrdered;

    let stream = vec![
        ok::<u32, ()>(1),
        ok::<u32, ()>(2),
        ok::<u32, ()>(3)
    ].into_iter().collect::<FuturesOrdered<_>>();
    assert_eq!(stream.len(), 3);
    assert_eq!(block_on(stream.collect()), Ok(vec![1,2,3]));
}

#[test]
fn queue_never_unblocked() {
    let (_a_tx, a_rx) = oneshot::channel::<Box<Any+Send>>();
    let (b_tx, b_rx) = oneshot::channel::<Box<Any+Send>>();
    let (c_tx, c_rx) = oneshot::channel::<Box<Any+Send>>();

    let mut stream = futures_ordered(vec![
        Box::new(a_rx) as Box<Future<Item = _, Error = _>>,
        Box::new(b_rx.select(c_rx).then(|res| Ok(Box::new(res) as Box<Any+Send>))) as _,
    ]);

    support::noop_waker_cx(|cx| {
        for _ in 0..10 {
            assert!(stream.poll_next(cx).unwrap().is_pending());
        }

        b_tx.send(Box::new(())).unwrap();
        assert!(stream.poll_next(cx).unwrap().is_pending());
        c_tx.send(Box::new(())).unwrap();
        assert!(stream.poll_next(cx).unwrap().is_pending());
        assert!(stream.poll_next(cx).unwrap().is_pending());
    })
}
