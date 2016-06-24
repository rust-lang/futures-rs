extern crate futures;

use std::sync::Arc;
use std::sync::mpsc::{Sender, channel};

use futures::*;

struct MyFuture {
    tx: Sender<Tokens>,
    txwake: Sender<Arc<Wake>>,
    tokens: Tokens,
}

impl Future for MyFuture {
    type Item = ();
    type Error = ();

    fn poll(&mut self, tokens: &Tokens) -> Option<PollResult<(), ()>> {
        self.tx.send(tokens.clone()).unwrap();
        None
    }

    fn schedule(&mut self, wake: Arc<Wake>) -> Tokens {
        drop(self.txwake.send(wake));
        self.tokens.clone()
    }

    fn tailcall(&mut self) -> Option<Box<Future<Item=(), Error=()>>> {
        None
    }
}

#[test]
fn select() {
    let (atx, arx) = channel();
    let (btx, brx) = channel();
    let (txwake, rxwake) = channel();
    let (donetx, donerx) = channel();

    MyFuture {
        tx: atx,
        txwake: txwake.clone(),
        tokens: Tokens::from_usize(2),
    }.select(MyFuture {
        tx: btx,
        txwake: txwake,
        tokens: Tokens::from_usize(4),
    }).map(move |x| donetx.send(x).unwrap()).forget();

    assert!(donerx.try_recv().is_err());

    // Figure out what our wake callback is
    let wake = rxwake.recv().unwrap();
    drop(rxwake);

    // May have a few poll() calls as part of the start to forget()
    while arx.try_recv().is_ok() {}
    while brx.try_recv().is_ok() {}

    // Send a wakeup notification which should only be for the right future
    wake.wake(&Tokens::from_usize(2));
    assert_eq!(arx.recv().unwrap(), Tokens::from_usize(2));
    assert!(brx.try_recv().is_err());
    wake.wake(&Tokens::from_usize(4));
    assert!(arx.try_recv().is_err());
    assert_eq!(brx.recv().unwrap(), Tokens::from_usize(4));

    // Send a wakeup with both
    wake.wake(&Tokens::all());
    assert_eq!(arx.recv().unwrap(), Tokens::from_usize(2));
    assert_eq!(brx.recv().unwrap(), Tokens::from_usize(4));
}

#[test]
fn join() {
    let (atx, arx) = channel();
    let (btx, brx) = channel();
    let (txwake, rxwake) = channel();
    let (donetx, donerx) = channel();

    MyFuture {
        tx: atx,
        txwake: txwake.clone(),
        tokens: Tokens::from_usize(2),
    }.join(MyFuture {
        tx: btx,
        txwake: txwake,
        tokens: Tokens::from_usize(4),
    }).map(move |x| donetx.send(x).unwrap()).forget();

    assert!(donerx.try_recv().is_err());

    // Figure out what our wake callback is
    let wake = rxwake.recv().unwrap();
    drop(rxwake);

    // May have a few poll() calls as part of the start to forget()
    while arx.try_recv().is_ok() {}
    while brx.try_recv().is_ok() {}

    // Send a wakeup notification which should only be for the right future
    wake.wake(&Tokens::from_usize(2));
    assert_eq!(arx.recv().unwrap(), Tokens::from_usize(2));
    assert!(brx.try_recv().is_err());
    wake.wake(&Tokens::from_usize(4));
    assert!(arx.try_recv().is_err());
    assert_eq!(brx.recv().unwrap(), Tokens::from_usize(4));

    // Send a wakeup with both
    wake.wake(&Tokens::all());
    assert_eq!(arx.recv().unwrap(), Tokens::from_usize(2));
    assert_eq!(brx.recv().unwrap(), Tokens::from_usize(4));
}
