extern crate futures;
extern crate futures_executor;
extern crate futures_channel;

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::thread;
use std::time::Duration;

use futures::future::lazy;
use futures::prelude::*;
use futures_executor::current_thread::*;
use futures_channel::oneshot;

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

    fn poll(&mut self, _: &mut TaskContext) -> Poll<(), ()> {
        Ok(Async::Pending)
    }
}

#[test]
fn outstanding_tasks_are_dropped_on_cancel() {
    let mut rc = Rc::new(());

    run(|_| {
        spawn(Never(rc.clone()));
        cancel_all_spawned();
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
fn tasks_are_scheduled_fairly() {
    let state = Rc::new(RefCell::new([0, 0]));

    struct Spin {
        state: Rc<RefCell<[i32; 2]>>,
        idx: usize,
    }

    impl Future for Spin {
        type Item = ();
        type Error = ();

        fn poll(&mut self, ctx: &mut TaskContext) -> Poll<(), ()> {
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

            ctx.waker().wake();
            Ok(Async::Pending)
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
