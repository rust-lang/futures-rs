#![feature(async_await, await_macro, futures_api, pin)]

use futures::{future, select};
use futures::stream::FuturesUnordered;
use futures::stream::StreamExt;
use futures_test::future::FutureTestExt;

#[test]
fn futures_unordered() {
    futures::executor::block_on(async {

        let mut fut = future::ready(1).pending_once();
        let mut async_tasks = FuturesUnordered::new();
        let mut total = 0;
        loop {
            select! {
                num = fut => {
                    // First, the `ready` future completes.
                    total += num;
                    // Then we spawn a new task onto `async_tasks`.
                    async_tasks.push(async { 5 });
                },
                // On the next iteration of the loop, the task we spawned
                // completes.
                num = async_tasks.next_some() => {
                    total += num;
                }
                // Finally, both the `ready` future and `async_tasks` have
                // finished, so we enter the `complete` branch.
                complete => break,
            }
        }
        assert_eq!(total, 6);
    });
}
