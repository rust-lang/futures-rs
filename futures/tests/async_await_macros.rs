#![recursion_limit="128"]
#![feature(async_await)]

use futures::{Poll, pending, pin_mut, poll, join, try_join, select};
use futures::channel::{mpsc, oneshot};
use futures::executor::block_on;
use futures::future::{self, FutureExt};
use futures::stream::StreamExt;
use futures::sink::SinkExt;

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
    let mut ran = false;
    block_on(async {
        select! {
            res = rx1.fuse() => {
                assert_eq!(Ok(1), res);
                ran = true;
            },
            _ = rx2.fuse() => unreachable!(),
        }
    });
    assert!(ran);
}

#[test]
fn select_streams() {
    let (mut tx1, rx1) = mpsc::channel::<i32>(1);
    let (mut tx2, rx2) = mpsc::channel::<i32>(1);
    let mut rx1 = rx1.fuse();
    let mut rx2 = rx2.fuse();
    let mut ran = false;
    let mut total = 0;
    block_on(async {
        let mut tx1_opt;
        let mut tx2_opt;
        select! {
            _ = rx1.next() => panic!(),
            _ = rx2.next() => panic!(),
            default => {
                tx1.send(2).await.unwrap();
                tx2.send(3).await.unwrap();
                tx1_opt = Some(tx1);
                tx2_opt = Some(tx2);
            }
            complete => panic!(),
        }
        loop {
            select! {
                // runs first and again after default
                x = rx1.next() => if let Some(x) = x { total += x; },
                // runs second and again after default
                x = rx2.next()  => if let Some(x) = x { total += x; },
                // runs third
                default => {
                    assert_eq!(total, 5);
                    ran = true;
                    drop(tx1_opt.take().unwrap());
                    drop(tx2_opt.take().unwrap());
                },
                // runs last
                complete => break,
            };
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
    let mut ran = false;
    let mut rx1 = rx1.fuse();
    let mut rx2 = rx2.fuse();
    block_on(async {
        select! {
            res = rx1 => {
                assert_eq!(Ok(1), res);
                assert_eq!(Ok(2), rx2.await);
                ran = true;
            },
            res = rx2 => {
                assert_eq!(Ok(2), res);
                assert_eq!(Ok(1), rx1.await);
                ran = true;
            },
        }
    });
    assert!(ran);
}

#[test]
fn select_nested() {
    let mut outer_fut = future::ready(1);
    let mut inner_fut = future::ready(2);
    let res = block_on(async {
        select! {
            x = outer_fut => {
                select! {
                    y = inner_fut => x + y,
                }
            }
        }
    });
    assert_eq!(res, 3);
}

#[test]
fn select_size() {
    let fut = async {
        let mut ready = future::ready(0i32);
        select! {
            _ = ready => {},
        }
    };
    assert_eq!(::std::mem::size_of_val(&fut), 24);

    let fut = async {
        let mut ready1 = future::ready(0i32);
        let mut ready2 = future::ready(0i32);
        select! {
            _ = ready1 => {},
            _ = ready2 => {},
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
    assert_eq!(::std::mem::size_of_val(&fut), 32);
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
    assert_eq!(::std::mem::size_of_val(&fut), 32);
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
