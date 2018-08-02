#![feature(async_await, await_macro, pin, arbitrary_self_types, futures_api)]

use futures::{Poll, future, pending, poll, pin_mut, join, try_join, select};
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

#[test]
fn select_size() {
    let fut = async {
        let mut ready = future::ready(0i32);
        select! {
            ready => {},
        }
    };
    assert_eq!(::std::mem::size_of_val(&fut), 40);

    let fut = async {
        let mut ready1 = future::ready(0i32);
        let mut ready2 = future::ready(0i32);
        select! {
            ready1 => {},
            ready2 => {},
        }
    };
    assert_eq!(::std::mem::size_of_val(&fut), 56);
}

#[test]
fn join_size() {
    let fut = async {
        let ready = future::ready(0i32);
        join!(ready)
    };
    assert_eq!(::std::mem::size_of_val(&fut), 48);

    let fut = async {
        let ready1 = future::ready(0i32);
        let ready2 = future::ready(0i32);
        join!(ready1, ready2)
    };
    assert_eq!(::std::mem::size_of_val(&fut), 72);
}

#[test]
fn try_join_size() {
    let fut = async {
        let ready = future::ready(Ok::<i32, i32>(0));
        try_join!(ready)
    };
    assert_eq!(::std::mem::size_of_val(&fut), 48);

    let fut = async {
        let ready1 = future::ready(Ok::<i32, i32>(0));
        let ready2 = future::ready(Ok::<i32, i32>(0));
        try_join!(ready1, ready2)
    };
    assert_eq!(::std::mem::size_of_val(&fut), 80);
}
