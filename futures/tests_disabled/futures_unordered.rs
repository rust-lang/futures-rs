extern crate futures;

use std::any::Any;

use futures::channel::oneshot;
use futures::executor::{block_on, block_on_stream};
use futures::stream::futures_unordered;
use futures::prelude::*;

mod support;

#[test]
fn works_1() {
    let (a_tx, a_rx) = oneshot::channel::<u32>();
    let (b_tx, b_rx) = oneshot::channel::<u32>();
    let (c_tx, c_rx) = oneshot::channel::<u32>();

    let mut stream = block_on_stream(futures_unordered(vec![a_rx, b_rx, c_rx]));

    b_tx.send(99).unwrap();
    assert_eq!(Some(Ok(99)), stream.next());

    a_tx.send(33).unwrap();
    c_tx.send(33).unwrap();
    assert_eq!(Some(Ok(33)), stream.next());
    assert_eq!(Some(Ok(33)), stream.next());
    assert_eq!(None, stream.next());
}

#[test]
fn works_2() {
    let (a_tx, a_rx) = oneshot::channel::<u32>();
    let (b_tx, b_rx) = oneshot::channel::<u32>();
    let (c_tx, c_rx) = oneshot::channel::<u32>();

    let mut stream = futures_unordered(vec![
        Box::new(a_rx) as Box<Future<Item = _, Error = _>>,
        Box::new(b_rx.join(c_rx).map(|(a, b)| a + b)),
    ]);

    a_tx.send(33).unwrap();
    b_tx.send(33).unwrap();
    support::noop_waker_cx(|cx| {
        assert!(stream.poll_next(cx).unwrap().is_ready());
        c_tx.send(33).unwrap();
        assert!(stream.poll_next(cx).unwrap().is_ready());
    })
}

#[test]
fn from_iterator() {
    use futures::future::ok;
    use futures::stream::FuturesUnordered;

    let stream = vec![
        ok::<u32, ()>(1),
        ok::<u32, ()>(2),
        ok::<u32, ()>(3)
    ].into_iter().collect::<FuturesUnordered<_>>();
    assert_eq!(stream.len(), 3);
    assert_eq!(block_on(stream.collect()), Ok(vec![1,2,3]));
}

#[test]
fn finished_future_ok() {
    let (_a_tx, a_rx) = oneshot::channel::<Box<Any+Send>>();
    let (b_tx, b_rx) = oneshot::channel::<Box<Any+Send>>();
    let (c_tx, c_rx) = oneshot::channel::<Box<Any+Send>>();

    let mut stream = futures_unordered(vec![
        Box::new(a_rx) as Box<Future<Item = _, Error = _>>,
        Box::new(b_rx.select(c_rx).then(|res| Ok(Box::new(res) as Box<Any+Send>))),
    ]);

    support::noop_waker_cx(|cx| {
        for _ in 0..10 {
            assert!(stream.poll_next(cx).unwrap().is_pending());
        }

        b_tx.send(Box::new(())).unwrap();
        let next = stream.poll_next(cx).unwrap();
        assert!(next.is_ready());
        c_tx.send(Box::new(())).unwrap();
        assert!(stream.poll_next(cx).unwrap().is_pending());
        assert!(stream.poll_next(cx).unwrap().is_pending());
    })
}

#[test]
fn iter_mut_cancel() {
    let (a_tx, a_rx) = oneshot::channel::<u32>();
    let (b_tx, b_rx) = oneshot::channel::<u32>();
    let (c_tx, c_rx) = oneshot::channel::<u32>();

    let mut stream = futures_unordered(vec![a_rx, b_rx, c_rx]);

    for rx in stream.iter_mut() {
        rx.close();
    }

    let mut stream = block_on_stream(stream);

    assert!(a_tx.is_canceled());
    assert!(b_tx.is_canceled());
    assert!(c_tx.is_canceled());

    assert_eq!(Some(Err(futures::channel::oneshot::Canceled)), stream.next());
    assert_eq!(Some(Err(futures::channel::oneshot::Canceled)), stream.next());
    assert_eq!(Some(Err(futures::channel::oneshot::Canceled)), stream.next());
    assert_eq!(None, stream.next());
}

#[test]
fn iter_mut_len() {
    let mut stream = futures_unordered(vec![
        futures::future::empty::<(),()>(),
        futures::future::empty::<(),()>(),
        futures::future::empty::<(),()>()
    ]);

    let mut iter_mut = stream.iter_mut();
    assert_eq!(iter_mut.len(), 3);
    assert!(iter_mut.next().is_some());
    assert_eq!(iter_mut.len(), 2);
    assert!(iter_mut.next().is_some());
    assert_eq!(iter_mut.len(), 1);
    assert!(iter_mut.next().is_some());
    assert_eq!(iter_mut.len(), 0);
    assert!(iter_mut.next().is_none());
}
