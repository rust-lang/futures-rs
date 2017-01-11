extern crate futures;

use std::cell::RefCell;
use std::rc::Rc;
use std::thread;

use futures::sync::oneshot;
use futures::Future;
use futures::future;

fn send_shared_oneshot_and_wait_on_multiple_threads(threads_number: u32) {
    let (tx, rx) = oneshot::channel::<u32>();
    let f = rx.shared();
    let threads = (0..threads_number).map(|_| {
        let cloned_future = f.clone();
        thread::spawn(move || {
            assert!(*cloned_future.wait().unwrap() == 6);
        })
    }).collect::<Vec<_>>();
    tx.complete(6);
    assert!(*f.wait().unwrap() == 6);
    for f in threads {
        f.join().unwrap();
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

#[test]
fn drop_in_poll() {
    let slot = Rc::new(RefCell::new(None));
    let slot2 = slot.clone();
    let future = future::poll_fn(move || {
        drop(slot2.borrow_mut().take().unwrap());
        Ok::<_, u32>(1.into())
    }).shared();
    let future2 = Box::new(future.clone()) as Box<Future<Item=_, Error=_>>;
    *slot.borrow_mut() = Some(future2);
    assert_eq!(*future.wait().unwrap(), 1);
}
