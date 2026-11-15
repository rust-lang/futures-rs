use std::{cell::Cell, task::Poll};

use futures::{
    executor::block_on,
    future::{FutureExt, err, ok, poll_fn, select_ok},
};
use futures_test::task::noop_context;

#[test]
fn ignore_err() {
    let v = vec![err(1), err(2), ok(3), ok(4)];

    let (i, v) = block_on(select_ok(v)).ok().unwrap();
    assert_eq!(i, 3);

    assert_eq!(v.len(), 1);

    let (i, v) = block_on(select_ok(v)).ok().unwrap();
    assert_eq!(i, 4);

    assert!(v.is_empty());
}

#[test]
fn last_err() {
    let v = vec![ok(1), err(2), err(3)];

    let (i, v) = block_on(select_ok(v)).ok().unwrap();
    assert_eq!(i, 1);

    assert_eq!(v.len(), 2);

    let i = block_on(select_ok(v)).err().unwrap();
    assert_eq!(i, 3);
}

#[test]
fn poll_pending_once() {
    let polls = &Cell::new(0);
    let pending = || {
        poll_fn(move |_| {
            polls.set(polls.get() + 1);
            Poll::<Result<(), ()>>::Pending
        })
        .boxed_local()
    };
    let v = vec![pending(), pending(), err(()).boxed_local(), err(()).boxed_local()];

    let mut f = select_ok(v);
    assert!(f.poll_unpin(&mut noop_context()).is_pending());
    assert_eq!(polls.get(), 2);
}
