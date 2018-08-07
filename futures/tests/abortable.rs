#![feature(pin, arbitrary_self_types, futures_api)]

use futures::FutureExt;
use futures::channel::oneshot;
use futures::executor::block_on;
use futures::future::{abortable, Aborted};
use futures::task::Poll;

mod support;
use self::support::with_counter_waker_context;

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

    with_counter_waker_context(|cx, counter| {
        assert_eq!(0, counter.get());
        assert_eq!(Poll::Pending, abortable_rx.poll_unpin(cx));
        assert_eq!(0, counter.get());
        abort_handle.abort();
        assert_eq!(1, counter.get());
        assert_eq!(Poll::Ready(Err(Aborted)), abortable_rx.poll_unpin(cx));
    })
}

#[test]
fn abortable_resolves() {
    let (tx, a_rx) = oneshot::channel::<()>();
    let (abortable_rx, _abort_handle) = abortable(a_rx);

    tx.send(()).unwrap();

    assert_eq!(Ok(Ok(())), block_on(abortable_rx));
}
