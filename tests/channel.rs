extern crate futures;

use std::sync::Arc;
use std::sync::atomic::*;

use futures::{Future, Async};
use futures::future::done;
use futures::executor;
use futures::stream::Stream;
use futures::sync::spsc;

mod support;
use support::*;

#[test]
fn sequence() {
    let (tx, mut rx) = spsc::channel();

    sassert_empty(&mut rx);
    sassert_empty(&mut rx);

    let amt = 20;
    send(amt, tx).forget();
    let mut rx = rx.wait();
    for i in (1..amt + 1).rev() {
        assert_eq!(rx.next(), Some(Ok(i)));
    }
    assert_eq!(rx.next(), None);

    fn send(n: u32, sender: spsc::Sender<u32, u32>)
            -> Box<Future<Item=(), Error=()> + Send> {
        if n == 0 {
            return done(Ok(())).boxed()
        }
        sender.send(Ok(n)).map_err(|_| ()).and_then(move |sender| {
            send(n - 1, sender)
        }).boxed()
    }
}

#[test]
fn drop_sender() {
    let (tx, mut rx) = spsc::channel::<u32, u32>();
    drop(tx);
    sassert_done(&mut rx);
}

#[test]
fn drop_rx() {
    let (tx, rx) = spsc::channel::<u32, u32>();
    let tx = tx.send(Ok(1)).wait().ok().unwrap();
    drop(rx);
    assert!(tx.send(Ok(1)).wait().is_err());
}

struct Unpark;

impl executor::Unpark for Unpark {
    fn unpark(&self) {
    }
}

#[test]
fn poll_future_then_drop() {
    let (tx, _rx) = spsc::channel::<u32, u32>();

    let tx = tx.send(Ok(1));
    let mut t = executor::spawn(tx);

    // First poll succeeds
    let tx = match t.poll_future(Arc::new(Unpark)) {
        Ok(Async::Ready(tx)) => tx,
        _ => panic!(),
    };

    // Send another value
    let tx = tx.send(Ok(2));
    let mut t = executor::spawn(tx);

    // Second poll doesn't
    match t.poll_future(Arc::new(Unpark)) {
        Ok(Async::NotReady) => {},
        _ => panic!(),
    };

    drop(t);
}

#[test]
fn drop_order() {
    static DROPS: AtomicUsize = ATOMIC_USIZE_INIT;
    let (tx, rx) = spsc::channel::<_, u32>();

    struct A;

    impl Drop for A {
        fn drop(&mut self) {
            DROPS.fetch_add(1, Ordering::SeqCst);
        }
    }

    let tx = tx.send(Ok(A)).wait().unwrap();
    assert_eq!(DROPS.load(Ordering::SeqCst), 0);
    drop(rx);
    assert_eq!(DROPS.load(Ordering::SeqCst), 1);
    assert!(tx.send(Ok(A)).wait().is_err());
    assert_eq!(DROPS.load(Ordering::SeqCst), 2);
}
