#![feature(async_await, await_macro, futures_api, pin)]

use futures::future;
use futures::task::Poll;
use futures::stream::{FusedStream, FuturesUnordered, StreamExt};
use futures_test::task::noop_local_waker_ref;

#[test]
fn is_terminated() {
    let lw = noop_local_waker_ref();
    let mut tasks = FuturesUnordered::new();

    assert_eq!(tasks.is_terminated(), false);
    assert_eq!(tasks.poll_next_unpin(lw), Poll::Ready(None));
    assert_eq!(tasks.is_terminated(), true);

    tasks.push(future::ready(1));

    assert_eq!(tasks.is_terminated(), false);
    assert_eq!(tasks.poll_next_unpin(lw), Poll::Ready(Some(1)));
    assert_eq!(tasks.is_terminated(), false);
    assert_eq!(tasks.poll_next_unpin(lw), Poll::Ready(None));
    assert_eq!(tasks.is_terminated(), true);
}
