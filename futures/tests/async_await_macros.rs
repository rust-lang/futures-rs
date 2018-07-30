#![feature(async_await, await_macro, pin, arbitrary_self_types, futures_api)]

use futures::{Poll, pending, poll, pin_mut, join, select};
use futures::channel::oneshot;
use futures::executor::block_on;

#[test]
fn poll_and_pending() {
    let pending_once = async { pending!() };
    block_on(async {
        pin_mut!(pending_once);
        assert_eq!(Poll::Pending, poll!(&mut pending_once));
        assert_eq!(Poll::Ready(()), poll!(&mut pending_once));
    });
}

#[test]
fn join() {
    let (tx1, rx1) = oneshot::channel::<i32>();
    let (tx2, rx2) = oneshot::channel::<i32>();

    let fut = async {
        let res = join!(rx1, rx2);
        assert_eq!((Ok(1), Ok(2)), res);
    };

    block_on(async {
        pin_mut!(fut);
        assert_eq!(Poll::Pending, poll!(&mut fut));
        tx1.send(1).unwrap();
        assert_eq!(Poll::Pending, poll!(&mut fut));
        tx2.send(2).unwrap();
        assert_eq!(Poll::Ready(()), poll!(&mut fut));
    });
}

#[test]
fn select() {
    let (tx1, mut rx1) = oneshot::channel::<i32>();
    let (_tx2, mut rx2) = oneshot::channel::<i32>();
    tx1.send(1).unwrap();
    let mut ran = false;
    block_on(async {
        select! {
            rx1 => {
                assert_eq!(Ok(1), rx1);
                ran = true;
            },
            rx2 => unreachable!(),
        }
    });
    assert!(ran);
}

#[test]
fn select_can_move_uncompleted_futures() {
    let (tx1, mut rx1) = oneshot::channel::<i32>();
    let (tx2, mut rx2) = oneshot::channel::<i32>();
    tx1.send(1).unwrap();
    tx2.send(2).unwrap();
    let mut ran = false;
    block_on(async {
        select! {
            rx1 => {
                assert_eq!(Ok(1), rx1);
                assert_eq!(Ok(2), await!(rx2));
                ran = true;
            },
            rx2 => {
                assert_eq!(Ok(2), rx2);
                assert_eq!(Ok(1), await!(rx1));
                ran = true;
            },
        }
    });
    assert!(ran);
}
