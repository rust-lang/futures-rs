extern crate futures;

mod support;

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

#[test]
fn polled_then_ignored() {
    let mut core = ::support::local_executor::Core::new();

    let (tx0, rx0) = oneshot::channel::<u32>();
    let f1 = rx0.shared();
    let f2 = f1.clone();

    let (tx1, rx1) = oneshot::channel::<u32>();
    let (tx2, rx2) = oneshot::channel::<u32>();
    let (tx3, rx3) = oneshot::channel::<u32>();

    core.spawn(f1.map(|n| tx3.complete(*n)).map_err(|_|()));

    core.run(future::ok::<(),()>(())).unwrap(); // Allow f1 to be polled.

    core.spawn(f2.map_err(|_| ()).map(|x| *x).select(rx2.map_err(|_| ())).map_err(|_| ())
        .and_then(|(_, f2)| rx3.map_err(|_| ()).map(move |n| {drop(f2); tx1.complete(n)})));

    core.run(future::ok::<(),()>(())).unwrap();  // Allow f2 to be polled.

    tx2.complete(11); // Resolve rx2, causing f2 to no longer get polled.

    core.run(future::ok::<(),()>(())).unwrap(); // Let the complete() propagate.

    tx0.complete(42); // Should cause f1, then rx3, and then rx1 to resolve.

    assert_eq!(core.run(rx1).unwrap(), 42);
}

#[test]
fn recursive_poll() {
    use futures::sync::mpsc;
    use futures::Stream;

    let mut core = ::support::local_executor::Core::new();
    let (tx0, rx0) = mpsc::unbounded::<Box<Future<Item=(),Error=()>>>();
    let run_stream = rx0.for_each(|f| f);

    let (tx1, rx1) = oneshot::channel::<()>();

    let f1 = run_stream.shared();
    let f2 = f1.clone();
    let f3 = f1.clone();
    tx0.send(Box::new(
        f1.map(|_|()).map_err(|_|())
            .select(rx1.map_err(|_|()))
            .map(|_| ()).map_err(|_|()))).unwrap();

    core.spawn(f2.map(|_|()).map_err(|_|()));

    // Call poll() on the spawned future. We want to be sure that this does not trigger a
    // deadlock or panic due to a recursive lock() on a mutex.
    core.run(future::ok::<(),()>(())).unwrap();

    tx1.complete(()); // Break the cycle.
    drop(tx0);
    core.run(f3).unwrap();
}
