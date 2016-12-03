extern crate futures;

use std::thread;
use futures::{oneshot, Future};


fn send_shared_oneshot_and_wait_on_multiple_threads(threads_number: u32) {
    let (tx, rx) = oneshot::<u32>();
    let f = rx.shared();
    let mut cloned_futures_waited_oneshots = vec![];
    for _ in 0..threads_number {
        let cloned_future = f.clone();
        let (tx2, rx2) = oneshot::<()>();
        cloned_futures_waited_oneshots.push(rx2);
        thread::spawn(move || {
            assert!(*cloned_future.wait().unwrap() == 6);
            tx2.complete(());
        });
    }
    tx.complete(6);
    for f in cloned_futures_waited_oneshots {
        f.wait().unwrap();
    }
}

#[test]
fn one_threads() {
    send_shared_oneshot_and_wait_on_multiple_threads(2);
}

#[test]
fn two_threads() {
    send_shared_oneshot_and_wait_on_multiple_threads(2);
}

#[test]
fn many_threads() {
    send_shared_oneshot_and_wait_on_multiple_threads(10000);
}
