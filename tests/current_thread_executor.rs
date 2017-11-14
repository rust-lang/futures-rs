extern crate futures;

use futures::Future;
use futures::future::lazy;
use futures::executor::CurrentThread;

use std::cell::Cell;
use std::rc::Rc;

#[test]
fn smoke() {
}

#[test]
fn spawning_from_init_future() {
    let cnt = Rc::new(Cell::new(0));

    CurrentThread::block_with_init(|| {
        let cnt = cnt.clone();

        CurrentThread::spawn(lazy(move || {
            cnt.set(1 + cnt.get());
            Ok(())
        }));
    });

    assert_eq!(1, cnt.get());
}

#[test]
fn block_waits_for_non_daemon() {
    use futures::sync::oneshot;
    use std::thread;
    use std::time::Duration;

    let cnt = Rc::new(Cell::new(0));

    CurrentThread::block_with_init(|| {
        let cnt = cnt.clone();

        let (tx, rx) = oneshot::channel();

        thread::spawn(|| {
            thread::sleep(Duration::from_millis(1000));
            tx.send(()).unwrap();
        });

        CurrentThread::spawn(rx.then(move |_| {
            cnt.set(1 + cnt.get());
            Ok(())
        }));
    });

    assert_eq!(1, cnt.get());
}

#[test]
fn spawning_future_from_initial_future() {
    let cnt = Rc::new(Cell::new(0));

    {
        let cnt = cnt.clone();

        CurrentThread::block_on_all(lazy(move || {
            CurrentThread::spawn(lazy(move || {
                cnt.set(1 + cnt.get());
                Ok(())
            }));
            Ok::<_, ()>(())
        })).unwrap();
    }

    assert_eq!(1, cnt.get());
}

#[test]
#[should_panic]
fn spawning_out_of_executor_context() {
    CurrentThread::spawn(lazy(|| Ok(())));
}
