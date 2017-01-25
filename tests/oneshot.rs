extern crate futures;

use std::sync::mpsc;
use std::thread;

use futures::{Future, Poll};
use futures::future::{lazy, ok};
use futures::sync::oneshot::*;
use futures::task::{self, Task};

mod support;
use support::*;

#[test]
fn smoke_poll() {
    let (mut tx, rx) = channel::<u32>();
    let mut task = futures::executor::spawn(lazy(|| {
        let task = task::empty();
        assert!(tx.poll_cancel(&task).unwrap().is_not_ready());
        assert!(tx.poll_cancel(&task).unwrap().is_not_ready());
        drop(rx);
        assert!(tx.poll_cancel(&task).unwrap().is_ready());
        assert!(tx.poll_cancel(&task).unwrap().is_ready());
        ok::<(), ()>(())
    }));
    assert!(task.poll_future(unpark_noop()).unwrap().is_ready());
}

#[test]
fn cancel_notifies() {
    let (tx, rx) = channel::<u32>();
    let (tx2, rx2) = mpsc::channel();

    WaitForCancel { tx: tx }.then(move |v| tx2.send(v)).forget();
    drop(rx);
    rx2.recv().unwrap().unwrap();
}

struct WaitForCancel {
    tx: Sender<u32>,
}

impl Future for WaitForCancel {
    type Item = ();
    type Error = ();

    fn poll(&mut self, task: &Task) -> Poll<(), ()> {
        self.tx.poll_cancel(task)
    }
}

#[test]
fn cancel_lots() {
    let (tx, rx) = mpsc::channel::<(Sender<_>, mpsc::Sender<_>)>();
    let t = thread::spawn(move || {
        for (tx, tx2) in rx {
            WaitForCancel { tx: tx }.then(move |v| tx2.send(v)).forget();
        }

    });

    for _ in 0..20000 {
        let (otx, orx) = channel::<u32>();
        let (tx2, rx2) = mpsc::channel();
        tx.send((otx, tx2)).unwrap();
        drop(orx);
        rx2.recv().unwrap().unwrap();
    }
    drop(tx);

    t.join().unwrap();
}

#[test]
fn close() {
    let (mut tx, mut rx) = channel::<u32>();
    rx.close();
    assert!(rx.poll(&task::empty()).is_err());
    assert!(tx.poll_cancel(&task::empty()).unwrap().is_ready());
}

#[test]
fn close_wakes() {
    let (tx, mut rx) = channel::<u32>();
    let (tx2, rx2) = mpsc::channel();
    let t = thread::spawn(move || {
        rx.close();
        rx2.recv().unwrap();
    });
    WaitForCancel { tx: tx }.wait().unwrap();
    tx2.send(()).unwrap();
    t.join().unwrap();
}
