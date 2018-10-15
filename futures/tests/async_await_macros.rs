#![feature(async_await, await_macro, pin, arbitrary_self_types, futures_api)]

use futures::{Poll, pending, poll, join, try_join, select};
use futures::channel::oneshot;
use futures::executor::block_on;
use futures::future::{self, FutureExt};
use pin_utils::pin_mut;

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
    let (tx1, rx1) = oneshot::channel::<i32>();
    let (_tx2, rx2) = oneshot::channel::<i32>();
    tx1.send(1).unwrap();
    let mut rx1 = rx1.fuse();
    let mut rx2 = rx2.fuse();
    let mut ran = false;
    block_on(async {
        select! {
            future(rx1 as res) => {
                assert_eq!(Ok(1), res);
                ran = true;
            },
            future(rx2 as _) => unreachable!(),
        }
    });
    assert!(ran);
}

#[test]
fn select_can_move_uncompleted_futures() {
    let (tx1, rx1) = oneshot::channel::<i32>();
    let (tx2, rx2) = oneshot::channel::<i32>();
    tx1.send(1).unwrap();
    tx2.send(2).unwrap();
    let mut rx1 = rx1.fuse();
    let mut rx2 = rx2.fuse();
    let mut ran = false;
    block_on(async {
        select! {
            future(rx1 as res) => {
                assert_eq!(Ok(1), res);
                assert_eq!(Ok(2), await!(rx2));
                ran = true;
            },
            future(rx2 as res) => {
                assert_eq!(Ok(2), res);
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
            future(ready as _) => {},
        }
    };
    assert_eq!(::std::mem::size_of_val(&fut), 24);

    let fut = async {
        let mut ready1 = future::ready(0i32);
        let mut ready2 = future::ready(0i32);
        select! {
            future(ready1 as _) => {},
            future(ready2 as _) => {},
        }
    };
    assert_eq!(::std::mem::size_of_val(&fut), 40);
}

#[test]
fn join_size() {
    let fut = async {
        let ready = future::ready(0i32);
        join!(ready)
    };
    assert_eq!(::std::mem::size_of_val(&fut), 16);

    let fut = async {
        let ready1 = future::ready(0i32);
        let ready2 = future::ready(0i32);
        join!(ready1, ready2)
    };
    assert_eq!(::std::mem::size_of_val(&fut), 44);
}

#[test]
fn try_join_size() {
    let fut = async {
        let ready = future::ready(Ok::<i32, i32>(0));
        try_join!(ready)
    };
    assert_eq!(::std::mem::size_of_val(&fut), 16);

    let fut = async {
        let ready1 = future::ready(Ok::<i32, i32>(0));
        let ready2 = future::ready(Ok::<i32, i32>(0));
        try_join!(ready1, ready2)
    };
    assert_eq!(::std::mem::size_of_val(&fut), 44);
}


#[test]
fn join_doesnt_require_unpin() {
    let _ = async {
        let x = async {};
        let y = async {};
        join!(x, y)
    };
}

#[test]
fn try_join_doesnt_require_unpin() {
    let _ = async {
        let x = async { Ok::<(), ()>(()) };
        let y = async { Ok::<(), ()>(()) };
        try_join!(x, y)
    };
}
