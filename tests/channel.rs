extern crate futures;

use futures::{done, Future};
use futures::stream::*;

mod support;
use support::*;

#[test]
fn sequence() {
    let (tx, mut rx) = channel();

    sassert_empty(&mut rx);
    sassert_empty(&mut rx);

    let amt = 20;
    send(amt, tx).forget();
    let mut rx = rx.wait();
    for i in (1..amt + 1).rev() {
        assert_eq!(rx.next(), Some(Ok(i)));
    }
    assert_eq!(rx.next(), None);

    fn send(n: u32, sender: Sender<u32, u32>)
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
    let (tx, mut rx) = channel::<u32, u32>();
    drop(tx);
    sassert_done(&mut rx);
}

#[test]
fn drop_rx() {
    let (tx, rx) = channel::<u32, u32>();
    let tx = tx.send(Ok(1)).wait().ok().unwrap();
    drop(rx);
    assert!(tx.send(Ok(1)).wait().is_err());
}
