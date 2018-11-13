#![feature(async_await, await_macro, futures_api, pin)]

use futures::{future, select};
use futures::stream::FuturesUnordered;
use futures::stream::StreamExt;
use futures_test::future::FutureTestExt;

#[test]
fn futures_unordered() {
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
