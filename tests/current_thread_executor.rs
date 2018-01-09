extern crate futures;

use futures::{task, Future, Poll, Async};
use futures::future::{self, blocking, empty, lazy};
use futures::executor::CurrentThread;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

fn inert() -> future::FutureResult<(), ()> {
    future::ok(())
}

#[test]
fn spawning_from_init_future() {
    let cnt = Rc::new(Cell::new(0));

    CurrentThread::run(|_| {
        let cnt = cnt.clone();

        CurrentThread::execute(lazy(move || {
            cnt.set(1 + cnt.get());
            Ok(())
        }));
        inert()
    }).unwrap();

    assert_eq!(1, cnt.get());
}

#[test]
fn block_waits_for_non_daemon() {
    use futures::sync::oneshot;
    use std::thread;
    use std::time::Duration;

    let cnt = Rc::new(Cell::new(0));

    CurrentThread::run(|_| {
        let cnt = cnt.clone();

        let (tx, rx) = oneshot::channel();

        thread::spawn(|| {
            thread::sleep(Duration::from_millis(1000));
            tx.send(()).unwrap();
        });

        CurrentThread::execute(rx.then(move |_| {
            cnt.set(1 + cnt.get());
            Ok(())
        }));
        inert()
    }).unwrap();

    assert_eq!(1, cnt.get());
}
//
#[test]
#[should_panic]
fn spawning_out_of_executor_context() {
    CurrentThread::execute(lazy(|| Ok(())));
}

#[test]
fn spawn_many() {
    const ITER: usize = 200;

    let cnt = Rc::new(Cell::new(0));

    CurrentThread::run(|_| {
        for _ in 0..ITER {
            let cnt = cnt.clone();
            CurrentThread::execute(lazy(move || {
                cnt.set(1 + cnt.get());
                Ok::<(), ()>(())
            }));
        }
        inert()
    }).unwrap();

    assert_eq!(cnt.get(), ITER);
}

struct Never(Rc<()>);

impl Future for Never {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Poll<(), ()> {
        Ok(Async::NotReady)
    }
}

#[test]
fn outstanding_daemon_tasks_are_dropped_on_return() {
    let mut rc = Rc::new(());

    CurrentThread::run(|_| {
        CurrentThread::execute_daemon(Never(rc.clone()));
        inert()
    }).unwrap();

    // Ensure the daemon is dropped
    assert!(Rc::get_mut(&mut rc).is_some());
}

#[test]
fn outstanding_tasks_are_dropped_on_cancel() {
    let mut rc = Rc::new(());

    CurrentThread::run(|_| {
        CurrentThread::execute(Never(rc.clone()));
        CurrentThread::cancel_all_executing();
        inert()
    }).unwrap();

    // Ensure the daemon is dropped
    assert!(Rc::get_mut(&mut rc).is_some());
}

#[test]
#[should_panic]
fn nesting_run() {
    CurrentThread::run(|_| {
        CurrentThread::run(|_| {
            inert()
        }).unwrap();
        inert()
    }).unwrap();
}

#[test]
#[should_panic]
fn run_in_future() {
    CurrentThread::run(|_| {
        CurrentThread::execute(lazy(|| {
            CurrentThread::run(|_| {
                inert()
            }).unwrap();
            Ok::<(), ()>(())
        }));
        inert()
    }).unwrap();
}

#[test]
#[should_panic]
fn blocking_within_init() {
    CurrentThread::run(|_| {
        let _ = blocking(empty::<(), ()>()).wait();
        inert()
    }).unwrap();
}

#[test]
#[should_panic]
fn blocking_in_future() {
    CurrentThread::run(|_| {
        CurrentThread::execute(lazy(|| {
            let _ = blocking(empty::<(), ()>()).wait();
            Ok::<(), ()>(())
        }));
        inert()
    }).unwrap();
}

#[test]
fn tasks_are_scheduled_fairly() {
    let state = Rc::new(RefCell::new([0, 0]));

    struct Spin {
        state: Rc<RefCell<[i32; 2]>>,
        idx: usize,
    }

    impl Future for Spin {
        type Item = ();
        type Error = ();

        fn poll(&mut self) -> Poll<(), ()> {
            let mut state = self.state.borrow_mut();

            if self.idx == 0 {
                let diff = state[0] - state[1];

                assert!(diff.abs() <= 1);

                if state[0] >= 50 {
                    return Ok(().into());
                }
            }

            state[self.idx] += 1;

            task::current().notify();
            Ok(Async::NotReady)
        }
    }

    CurrentThread::run(|_| {
        CurrentThread::execute(Spin {
            state: state.clone(),
            idx: 0,
        });

        CurrentThread::execute_daemon(Spin {
            state: state,
            idx: 1,
        });
        inert()
    }).unwrap();
}
