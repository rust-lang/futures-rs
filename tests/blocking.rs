extern crate futures;

use futures::future::blocking;
use futures::unsync::oneshot;

#[test]
fn future_try_take() {
    let (tx, rx) = oneshot::channel::<u32>();
    let mut rx = blocking(rx);

    assert!(rx.try_take().is_none());

    tx.send(1).unwrap();

    assert_eq!(Some(Ok(1)), rx.try_take());
}
