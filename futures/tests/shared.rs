extern crate futures;

mod support;

use std::cell::RefCell;
use std::rc::Rc;
use std::thread;

use futures::channel::oneshot;
use futures::executor::{block_on, LocalPool};
use futures::prelude::*;
use futures::future;

fn send_shared_oneshot_and_wait_on_multiple_threads(threads_number: u32) {
    let (tx, rx) = oneshot::channel::<u32>();
    let f = rx.shared();
    let threads = (0..threads_number).map(|_| {
        let cloned_future = f.clone();
        thread::spawn(move || {
            assert_eq!(*block_on(cloned_future).unwrap(), 6);
        })
    }).collect::<Vec<_>>();
    tx.send(6).unwrap();
    assert_eq!(*block_on(f).unwrap(), 6);
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
        drop(block_on(f));
    });

    let (tx3, rx3) = oneshot::channel::<u32>();

    let t2 = thread::spawn(|| {
        let _ = block_on(f2.map(|x| tx3.send(*x).unwrap()).map_err(|_| ()));
    });

    tx2.send(11).unwrap(); // cancel `f1`
    t1.join().unwrap();

    tx.send(42).unwrap(); // Should cause `f2` and then `rx3` to get resolved.
    let result = block_on(rx3).unwrap();
    assert_eq!(result, 42);
    t2.join().unwrap();
}

#[test]
fn drop_in_poll() {
    let slot = Rc::new(RefCell::new(None));
    let slot2 = slot.clone();
    let future = future::poll_fn(move |_| {
        drop(slot2.borrow_mut().take().unwrap());
        Ok::<_, u32>(1.into())
    }).shared();
    let future2 = Box::new(future.clone()) as Box<Future<Item=_, Error=_>>;
    *slot.borrow_mut() = Some(future2);
    assert_eq!(*block_on(future).unwrap(), 1);
}

#[test]
fn peek() {
    let mut core = LocalPool::new();
    let exec = &mut core.executor();

    let (tx0, rx0) = oneshot::channel::<u32>();
    let f1 = rx0.shared();
    let f2 = f1.clone();

    // Repeated calls on the original or clone do not change the outcome.
    for _ in 0..2 {
        assert!(f1.peek().is_none());
        assert!(f2.peek().is_none());
    }

    // Completing the underlying future has no effect, because the value has not been `poll`ed in.
    tx0.send(42).unwrap();
    for _ in 0..2 {
        assert!(f1.peek().is_none());
        assert!(f2.peek().is_none());
    }

    // Once the Shared has been polled, the value is peekable on the clone.
    exec.spawn(Box::new(f1.map(|_|()).recover(|_|()))).unwrap();
    core.run(exec);
    for _ in 0..2 {
        assert_eq!(42, *f2.peek().unwrap().unwrap());
    }
}
