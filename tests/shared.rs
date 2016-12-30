extern crate futures;

use std::thread;

use futures::sync::oneshot;
use futures::Future;

fn send_shared_oneshot_and_wait_on_multiple_threads(threads_number: u32) {
    let (tx, rx) = oneshot::channel::<u32>();
    let f = rx.shared();
    let mut cloned_futures_waited_oneshots = vec![];
    for _ in 0..threads_number {
        let cloned_future = f.clone();
        let (tx2, rx2) = oneshot::channel::<()>();
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
fn one_thread() {
    send_shared_oneshot_and_wait_on_multiple_threads(1);
}

#[test]
fn two_threads() {
    send_shared_oneshot_and_wait_on_multiple_threads(2);
}

#[test]
fn many_threads() {
    send_shared_oneshot_and_wait_on_multiple_threads(1000);
}

#[test]
fn drop_on_one_task_ok() {
    let (tx, rx) = oneshot::channel::<u32>();
    let f1 = rx.shared();
    let f2 = f1.clone();

    let (tx2, rx2) = oneshot::channel::<u32>();

    let t1 = thread::spawn(|| {
        let f = f1.map_err(|_| ()).map(|x| *x).select(rx2.map_err(|_| ()));
        drop(f.wait());
    });

    let (tx3, rx3) = oneshot::channel::<u32>();

    let t2 = thread::spawn(|| {
        drop(f2.map(|x| tx3.complete(*x)).map_err(|_| ()).wait());
    });

    tx2.complete(11); // cancel `f1`
    t1.join().unwrap();

    tx.complete(42); // Should cause `f2` and then `rx3` to get resolved.
    let result = rx3.wait().unwrap();
    assert_eq!(result, 42);
    t2.join().unwrap();
}
