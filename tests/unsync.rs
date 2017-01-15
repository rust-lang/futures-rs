#![cfg(feature = "use_std")]

extern crate futures;

use futures::{Future, Stream, Sink, Async};
use futures::sink::SendError;
use futures::unsync::mpsc;
use futures::future::lazy;
use futures::stream::iter;

#[test]
fn mpsc_send_recv() {
    let (tx, rx) = mpsc::channel::<i32>(1);
    let mut rx = rx.wait();

    tx.send(42).wait().unwrap();

    assert_eq!(rx.next(), Some(Ok(42)));
    assert_eq!(rx.next(), None);
}

#[test]
fn mpsc_rx_notready() {
    let (_tx, mut rx) = mpsc::channel::<i32>(1);

    lazy(|| {
        assert_eq!(rx.poll().unwrap(), Async::NotReady);
        Ok(()) as Result<(), ()>
    }).wait().unwrap();
}

#[test]
fn mpsc_rx_end() {
    let (_, mut rx) = mpsc::channel::<i32>(1);

    lazy(|| {
        assert_eq!(rx.poll().unwrap(), Async::Ready(None));
        Ok(()) as Result<(), ()>
    }).wait().unwrap();
}

#[test]
fn mpsc_tx_notready() {
    let (tx, _rx) = mpsc::channel::<i32>(1);
    let tx = tx.send(1).wait().unwrap();
    lazy(move || {
        assert!(tx.send(2).poll().unwrap().is_not_ready());
        Ok(()) as Result<(), ()>
    }).wait().unwrap();
}

#[test]
fn mpsc_tx_err() {
    let (tx, _) = mpsc::channel::<i32>(1);
    lazy(move || {
        assert!(tx.send(2).poll().is_err());
        Ok(()) as Result<(), ()>
    }).wait().unwrap();
}

#[test]
fn mpsc_backpressure() {
    let (tx, rx) = mpsc::channel::<i32>(1);
    lazy(move || {
        iter(vec![1, 2, 3].into_iter().map(Ok))
            .forward(tx)
            .map_err(|e: SendError<i32>| panic!("{}", e))
            .join(rx.take(3).collect().map(|xs| {
                assert!(xs == [1, 2, 3]);
            }))
    }).wait().unwrap();
}
