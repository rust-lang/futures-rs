#![allow(unused_imports)]

extern crate futures;
extern crate futures_executor;
extern crate futures_channel;

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::thread;
use std::time::Duration;

use futures::future::lazy;
use futures::prelude::*;
use futures::executor::Executor;
use futures::task;
use futures_executor::*;
use futures_channel::oneshot;

struct Pending(Rc<()>);

impl Future for Pending {
    type Item = ();
    type Error = Never;

    fn poll(&mut self, _: &mut task::Context) -> Poll<(), Never> {
        Ok(Async::Pending)
    }
}

fn pending() -> Pending {
    Pending(Rc::new(()))
}

const DONE: Result<(), Never> = Ok(());

#[test]
fn run_until_single_future() {
    let mut cnt = 0;

    {
        let mut pool = LocalPool::new();
        let mut exec = pool.executor();
        let fut = lazy(|| {
            cnt += 1;
            DONE
        });
        pool.run_until(fut, &mut exec).unwrap();
    }

    assert_eq!(cnt, 1);
}

#[test]
fn run_until_ignores_spawned() {
    let mut pool = LocalPool::new();
    let mut exec = pool.executor();
    exec.spawn_local(Box::new(pending())).unwrap();
    pool.run_until(lazy(|| DONE), &mut exec).unwrap();
}

#[test]
fn run_until_executes_spawned() {
    let (tx, rx) = oneshot::channel();
    let mut pool = LocalPool::new();
    let mut exec = pool.executor();
    exec.spawn_local(Box::new(lazy(move || {
        tx.send(()).unwrap();
        DONE
    }))).unwrap();
    pool.run_until(rx, &mut exec).unwrap();
}

#[test]
fn run_executes_spawned() {
    let cnt = Rc::new(Cell::new(0));
    let cnt2 = cnt.clone();

    let mut pool = LocalPool::new();
    let mut exec = pool.executor();
    let mut exec2 = pool.executor();

    exec.spawn_local(Box::new(lazy(move || {
        exec2.spawn_local(Box::new(lazy(move || {
            cnt2.set(cnt2.get() + 1);
            DONE
        }))).unwrap();
        DONE
    }))).unwrap();

    pool.run(&mut exec);

    assert_eq!(cnt.get(), 1);
}

#[test]
fn run_spawn_many() {
    const ITER: usize = 200;

    let cnt = Rc::new(Cell::new(0));

    let mut pool = LocalPool::new();
    let mut exec = pool.executor();

    for _ in 0..ITER {
        let cnt = cnt.clone();
        exec.spawn_local(Box::new(lazy(move || {
            cnt.set(cnt.get() + 1);
            DONE
        }))).unwrap();
    }

    pool.run(&mut exec);

    assert_eq!(cnt.get(), ITER);
}

#[test]
#[should_panic]
fn nesting_run() {
    let mut pool = LocalPool::new();
    let mut exec = pool.executor();

    exec.spawn(Box::new(lazy(|| {
        let mut pool = LocalPool::new();
        let mut exec = pool.executor();
        pool.run(&mut exec);
        Ok(())
    }))).unwrap();
    pool.run(&mut exec);
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
        type Error = Never;

        fn poll(&mut self, cx: &mut task::Context) -> Poll<(), Never> {
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

            cx.waker().wake();
            Ok(Async::Pending)
        }
    }

    let mut pool = LocalPool::new();
    let mut exec = pool.executor();

    exec.spawn_local(Box::new(Spin {
        state: state.clone(),
        idx: 0,
    })).unwrap();

    exec.spawn_local(Box::new(Spin {
        state: state,
        idx: 1,
    })).unwrap();

    pool.run(&mut exec);
}
