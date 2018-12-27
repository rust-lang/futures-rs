#![feature(futures_api)]

use futures::channel::oneshot;
use futures::executor::block_on;
use futures::future::{abortable, Aborted, FutureExt};
use futures::task::Poll;
use futures_test::task::WakeCounter;

#[test]
fn abortable_works() {
    let (_tx, a_rx) = oneshot::channel::<()>();
    let (abortable_rx, abort_handle) = abortable(a_rx);

    abort_handle.abort();
    assert_eq!(Err(Aborted), block_on(abortable_rx));
}

#[test]
fn abortable_awakens() {
    let (_tx, a_rx) = oneshot::channel::<()>();
    let (mut abortable_rx, abort_handle) = abortable(a_rx);

    let wake_counter = WakeCounter::new();
    let lw = &wake_counter.local_waker();
    assert_eq!(0, wake_counter.count());
    assert_eq!(Poll::Pending, abortable_rx.poll_unpin(lw));
    assert_eq!(0, wake_counter.count());
    abort_handle.abort();
    assert_eq!(1, wake_counter.count());
    assert_eq!(Poll::Ready(Err(Aborted)), abortable_rx.poll_unpin(lw));
}

#[test]
fn abortable_resolves() {
    let (tx, a_rx) = oneshot::channel::<()>();
    let (abortable_rx, _abort_handle) = abortable(a_rx);

    tx.send(()).unwrap();

    assert_eq!(Ok(Ok(())), block_on(abortable_rx));
}
