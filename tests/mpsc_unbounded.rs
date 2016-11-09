extern crate futures;

use futures::sync::mpsc::*;
use std::sync::atomic::*;
use futures::stream::Stream;
use std::thread;

mod support;
use support::*;

fn send(n: u32, mut sender: Sender<u32>) {
    if n == 0 {
        return;
    }
    sender.send(n).unwrap();
    send(n - 1, sender)
}

#[test]
fn multiple_senders() {
    let (tx, rx) = unbounded();

    let tx2 = tx.clone();
    let tx3 = tx.clone();
    let amt = 40;
    thread::spawn(move || send(amt, tx));
    thread::spawn(move || send(amt, tx2));
    thread::spawn(move || send(amt, tx3));
    let mut rx = rx.wait();
    for _ in 1..(amt * 3 + 1) {
        assert!(rx.next().is_some());
    }
    assert_eq!(rx.next(), None);
}

#[test]
fn sequence() {
    let (tx, mut rx) = unbounded();

    sassert_empty(&mut rx);
    sassert_empty(&mut rx);

    let amt = 20;
    send(amt, tx);
    let mut rx = rx.wait();
    for i in (1..amt + 1).rev() {
        assert_eq!(rx.next(), Some(Ok(i)));
    }
    assert_eq!(rx.next(), None);
}

#[test]
fn drop_sender() {
    let (tx, mut rx) = unbounded::<()>();
    drop(tx);
    sassert_done(&mut rx);
}

#[test]
fn drop_rx() {
    let (mut tx, rx) = unbounded::<u32>();
    tx.send(1).unwrap();
    drop(rx);
    assert!(tx.send(1).is_err());
}

#[test]
fn drop_order() {
    static DROPS: AtomicUsize = ATOMIC_USIZE_INIT;
    let (mut tx, rx) = unbounded();

    struct A;

    impl Drop for A {
        fn drop(&mut self) {
            DROPS.fetch_add(1, Ordering::SeqCst);
        }
    }

    tx.send(A).unwrap();
    assert_eq!(DROPS.load(Ordering::SeqCst), 0);
    drop(rx);
    assert_eq!(DROPS.load(Ordering::SeqCst), 1);
    assert!(tx.send(A).is_err());
    assert_eq!(DROPS.load(Ordering::SeqCst), 2);
}
