#![feature(async_await, await_macro, futures_api, pin)]

use futures::future;
use futures::FutureExt;
use futures::task::Poll;
use futures::stream::FusedStream;
use futures::stream::{SelectAll, StreamExt};
use futures_test::task::noop_local_waker_ref;

#[test]
fn is_terminated() {
    let lw = noop_local_waker_ref();
    let mut tasks = SelectAll::new();

    assert_eq!(tasks.is_terminated(), false);
    assert_eq!(tasks.poll_next_unpin(lw), Poll::Ready(None));
    assert_eq!(tasks.is_terminated(), true);

    // Test that the sentinel value doesn't leak
    assert_eq!(tasks.is_empty(), true);
    assert_eq!(tasks.len(), 0);

    tasks.push(future::ready(1).into_stream());

    assert_eq!(tasks.is_empty(), false);
    assert_eq!(tasks.len(), 1);

    assert_eq!(tasks.is_terminated(), false);
    assert_eq!(tasks.poll_next_unpin(lw), Poll::Ready(Some(1)));
    assert_eq!(tasks.is_terminated(), false);
    assert_eq!(tasks.poll_next_unpin(lw), Poll::Ready(None));
    assert_eq!(tasks.is_terminated(), true);
}
