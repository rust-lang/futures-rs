#![feature(async_await, await_macro, futures_api)]

use futures::future;
use futures::task::{Context, Poll};
use futures::stream::{FusedStream, FuturesUnordered, StreamExt};
use futures_test::task::noop_waker_ref;

#[test]
fn is_terminated() {
    let mut cx = Context::from_waker(noop_waker_ref());
    let mut tasks = FuturesUnordered::new();

    assert_eq!(tasks.is_terminated(), false);
    assert_eq!(tasks.poll_next_unpin(&mut cx), Poll::Ready(None));
    assert_eq!(tasks.is_terminated(), true);

    // Test that the sentinel value doesn't leak
    assert_eq!(tasks.is_empty(), true);
    assert_eq!(tasks.len(), 0);
    assert_eq!(tasks.iter_mut().len(), 0);

    tasks.push(future::ready(1));

    assert_eq!(tasks.is_empty(), false);
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks.iter_mut().len(), 1);

    assert_eq!(tasks.is_terminated(), false);
    assert_eq!(tasks.poll_next_unpin(&mut cx), Poll::Ready(Some(1)));
    assert_eq!(tasks.is_terminated(), false);
    assert_eq!(tasks.poll_next_unpin(&mut cx), Poll::Ready(None));
    assert_eq!(tasks.is_terminated(), true);
}
