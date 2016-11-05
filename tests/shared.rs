extern crate futures;

use std::thread;
use futures::{oneshot, Future};

#[test]
fn two_threads() {
    let (tx, rx) = oneshot::<u32>();
    let f1 = rx.shared();
    let f2 = f1.clone();

    let (tx1, rx1) = oneshot::<()>();
    let (tx2, rx2) = oneshot::<()>();

    thread::spawn(move || {
        assert!(*f1.wait().unwrap() == 3);
        tx1.complete(());
    });
    thread::spawn(move || {
        assert!(*f2.wait().unwrap() == 3);
        tx2.complete(());
    });

    thread::spawn(|| {
        tx.complete(3);
    });

    rx1.wait().unwrap();
    rx2.wait().unwrap();
}


#[test]
fn many_threads() {
    let (tx, rx) = oneshot::<u32>();
    let f = rx.shared();
    let mut cloned_futures_waited_oneshots = vec![];
    for _ in 0..10000 {
        let cloned_future = f.clone();
        let (tx2, rx2) = oneshot::<()>();
        cloned_futures_waited_oneshots.push(rx2);
        thread::spawn(move || {
            assert!(*cloned_future.wait().unwrap() == 3);
            tx2.complete(());
        });
    }
    tx.complete(3);
    for f in cloned_futures_waited_oneshots {
        f.wait().unwrap();
    }
}
