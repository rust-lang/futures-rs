extern crate futures;

use std::sync::mpsc::{channel, Sender};
use std::thread;

use futures::{oneshot, Complete, Future, Poll};

mod support;
use support::*;

#[test]
fn smoke_poll() {
    let (mut tx, rx) = oneshot::<u32>();
    let mut task = futures::task::spawn(futures::lazy(|| {
        assert!(tx.poll_cancel().unwrap().is_not_ready());
        assert!(tx.poll_cancel().unwrap().is_not_ready());
        drop(rx);
        assert!(tx.poll_cancel().unwrap().is_ready());
        assert!(tx.poll_cancel().unwrap().is_ready());
        futures::finished::<(), ()>(())
    }));
    assert!(task.poll_future(unpark_noop()).unwrap().is_ready());
}

#[test]
fn cancel_notifies() {
    let (tx, rx) = oneshot::<u32>();
    let (tx2, rx2) = channel();

    WaitForCancel { tx: tx }.then(move |v| tx2.send(v)).forget();
    drop(rx);
    rx2.recv().unwrap().unwrap();
}

struct WaitForCancel {
    tx: Complete<u32>,
}

impl Future for WaitForCancel {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Poll<(), ()> {
        self.tx.poll_cancel()
    }
}

#[test]
fn cancel_lots() {
    let (tx, rx) = channel::<(Complete<_>, Sender<_>)>();
    let t = thread::spawn(move || {
        for (tx, tx2) in rx {
            WaitForCancel { tx: tx }.then(move |v| tx2.send(v)).forget();
        }

    });

    for _ in 0..20000 {
        let (otx, orx) = oneshot::<u32>();
        let (tx2, rx2) = channel();
        tx.send((otx, tx2)).unwrap();
        drop(orx);
        rx2.recv().unwrap().unwrap();
    }
    drop(tx);

    t.join().unwrap();
}
