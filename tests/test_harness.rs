extern crate futures;

use futures::{future, Async};
use futures::executor::TestHarness;
use futures::sync::oneshot;

use std::thread;
use std::time::Duration;

#[test]
fn poll_ready_future() {
    let mut harness = TestHarness::new(future::ok::<_, ()>("hello"));
    assert_eq!(Ok(Async::Ready("hello")), harness.poll());
}

#[test]
fn poll_not_ready_future() {
    let (tx, rx) = oneshot::channel();

    let mut harness = TestHarness::new(rx);

    assert_eq!(Ok(Async::NotReady), harness.poll());

    // Not notified
    assert!(!harness.is_notified());

    // Complete
    tx.send("hello").unwrap();

    // Is notified
    assert!(harness.is_notified());

    assert_eq!(Ok(Async::Ready("hello")), harness.poll());
}

#[test]
fn wait_unbounded() {
    let (tx, rx) = oneshot::channel();

    let th = thread::spawn(move || {
        thread::sleep(Duration::from_secs(1));
        tx.send("hello").unwrap();
    });

    let mut harness = TestHarness::new(rx);

    // Wait for it
    assert_eq!("hello", harness.wait().unwrap());

    th.join().unwrap();
}

#[test]
fn wait_bounded() {
    let rx = future::empty::<(), ()>();

    let mut harness = TestHarness::new(rx);

    // Wait for it.. for a bit
    let res = harness.wait_timeout(Duration::from_millis(300));

    assert!(res.unwrap_err().is_timeout());
}
