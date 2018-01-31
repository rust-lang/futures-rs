extern crate futures;

use futures::{task, Future, Poll, Async};
use futures::future::{blocking, empty, lazy};
use futures::current_thread::*;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

#[test]
fn spawning_from_init_future() {
    let cnt = Rc::new(Cell::new(0));

    run(|_| {
        let cnt = cnt.clone();

        spawn(lazy(move || {
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

    run(|_| {
        let cnt = cnt.clone();

        let (tx, rx) = oneshot::channel();

        thread::spawn(|| {
            thread::sleep(Duration::from_millis(1000));
            tx.send(()).unwrap();
        });

        spawn(rx.then(move |_| {
            cnt.set(1 + cnt.get());
            Ok(())
        }));
    });

    assert_eq!(1, cnt.get());
}

#[test]
#[should_panic]
fn spawning_out_of_executor_context() {
    spawn(lazy(|| Ok(())));
}

#[test]
fn spawn_many() {
    const ITER: usize = 200;

    let cnt = Rc::new(Cell::new(0));

    run(|_| {
        for _ in 0..ITER {
            let cnt = cnt.clone();
            spawn(lazy(move || {
                cnt.set(1 + cnt.get());
                Ok::<(), ()>(())
            }));
        }
    });

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
fn outstanding_tasks_are_dropped_on_cancel() {
    let mut rc = Rc::new(());

    run(|ctx| {
        spawn(Never(rc.clone()));
        ctx.cancel_all_spawned();
    });

    // Ensure the daemon is dropped
    assert!(Rc::get_mut(&mut rc).is_some());
}

#[test]
#[should_panic]
fn nesting_run() {
    run(|_| {
        run(|_| {
        });
    });
}

#[test]
#[should_panic]
fn run_in_future() {
    run(|_| {
        spawn(lazy(|| {
            run(|_| {
            });
            Ok::<(), ()>(())
        }));
    });
}

#[test]
#[should_panic]
fn blocking_within_init() {
    run(|_| {
        let _ = blocking(empty::<(), ()>()).wait();
    });
}

#[test]
#[should_panic]
fn blocking_in_future() {
    run(|_| {
        spawn(lazy(|| {
            let _ = blocking(empty::<(), ()>()).wait();
            Ok::<(), ()>(())
        }));
    });
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

            if state[self.idx] >= 100 {
                return Ok(().into());
            }

            task::current().notify();
            Ok(Async::NotReady)
        }
    }

    run(|_| {
        spawn(Spin {
            state: state.clone(),
            idx: 0,
        });

        spawn(Spin {
            state: state,
            idx: 1,
        });
    });
}
