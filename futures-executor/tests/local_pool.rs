#![allow(unused_imports)]

#![feature(pin, arbitrary_self_types)]

extern crate futures;
extern crate futures_executor;
extern crate futures_channel;

use std::cell::{Cell, RefCell};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use std::mem::PinMut;

use futures::future::lazy;
use futures::prelude::*;
use futures::executor::Executor;
use futures::task;
use futures_executor::*;
use futures_channel::oneshot;

struct Pending(Arc<()>);

impl Future for Pending {
    type Output = ();

    fn poll(self: PinMut<Self>, _cx: &mut task::Context) -> Poll<()> {
        Poll::Pending
    }
}

fn pending() -> Pending {
    Pending(Arc::new(()))
}

#[test]
fn run_until_single_future() {
    let mut cnt = 0;

    {
        let mut pool = LocalPool::new();
        let mut exec = pool.executor();
        let fut = lazy(|_| {
            cnt += 1;
            ()
        });
        assert_eq!(pool.run_until(fut, &mut exec), ());
    }

    assert_eq!(cnt, 1);
}

#[test]
fn run_until_ignores_spawned() {
    let mut pool = LocalPool::new();
    let mut exec = pool.executor();
    exec.spawn_obj(Box::new(pending()).into()).unwrap(); // This test used the currently not implemented spawn_local method before
    assert_eq!(pool.run_until(lazy(|_| ()), &mut exec), ());
}

#[test]
fn run_until_executes_spawned() {
    let (tx, rx) = oneshot::channel();
    let mut pool = LocalPool::new();
    let mut exec = pool.executor();
    exec.spawn_obj(Box::new(lazy(move |_| { // This test used the currently not implemented spawn_local method before
        tx.send(()).unwrap();
        ()
    })).into()).unwrap();
    pool.run_until(rx, &mut exec).unwrap();
}

/* // This test does not work because it relies on spawn_local which is not implemented
#[test]
fn run_executes_spawned() {
    let cnt = Rc::new(Cell::new(0));
    let cnt2 = cnt.clone();

    let mut pool = LocalPool::new();
    let mut exec = pool.executor();
    let mut exec2 = pool.executor();

    exec.spawn_local(Box::new(lazy(move |_| {
        exec2.spawn_local(Box::new(lazy(move |_| {
            cnt2.set(cnt2.get() + 1);
            ()
        })).into()).unwrap();
        ()
    })).into()).unwrap();

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
        exec.spawn_local(Box::new(lazy(move |_| {
            cnt.set(cnt.get() + 1);
            ()
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

    exec.spawn(Box::new(lazy(|_| {
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
        type Output = ();

        fn poll(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<()> {
            let mut state = self.state.borrow_mut();

            if self.idx == 0 {
                let diff = state[0] - state[1];

                assert!(diff.abs() <= 1);

                if state[0] >= 50 {
                    return Poll::Ready(());
                }
            }

            state[self.idx] += 1;

            if state[self.idx] >= 100 {
                return Poll::Ready(());
            }

            cx.waker().wake();
            Poll::Pending
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
*/
