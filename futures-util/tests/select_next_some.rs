#![feature(async_await, await_macro, futures_api)]

use futures::{future, select};
use futures::future::{FusedFuture, FutureExt};
use futures::stream::{FuturesUnordered, StreamExt};
use futures::task::Poll;
use futures_test::future::FutureTestExt;
use futures_test::task::WakeCounter;

#[test]
fn is_terminated() {
    let counter = WakeCounter::new();

    let mut tasks = FuturesUnordered::new();

    let mut select_next_some = tasks.select_next_some();
    assert_eq!(select_next_some.is_terminated(), false);
    assert_eq!(select_next_some.poll_unpin(counter.local_waker()), Poll::Pending);
    assert_eq!(counter.count(), 1);
    assert_eq!(select_next_some.is_terminated(), true);
    drop(select_next_some);

    tasks.push(future::ready(1));

    let mut select_next_some = tasks.select_next_some();
    assert_eq!(select_next_some.is_terminated(), false);
    assert_eq!(select_next_some.poll_unpin(counter.local_waker()), Poll::Ready(1));
    assert_eq!(select_next_some.is_terminated(), false);
    assert_eq!(select_next_some.poll_unpin(counter.local_waker()), Poll::Pending);
    assert_eq!(select_next_some.is_terminated(), true);
}

#[test]
fn select() {
    // Checks that even though `async_tasks` will yield a `None` and return
    // `is_terminated() == true` during the first poll, it manages to toggle
    // back to having items after a future is pushed into it during the second
    // poll (after pending_once completes).
    futures::executor::block_on(async {
        let mut fut = future::ready(1).pending_once();
        let mut async_tasks = FuturesUnordered::new();
        let mut total = 0;
        loop {
            select! {
                num = fut => {
                    total += num;
                    async_tasks.push(async { 5 });
                },
                num = async_tasks.select_next_some() => {
                    total += num;
                }
                complete => break,
            }
        }
        assert_eq!(total, 6);
    });
}
