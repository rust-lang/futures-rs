use std::fmt::Debug;
use std::time::Duration;

use futures::executor::block_on;
use futures::future::{err, ok, select_ok, select_ok_biased, Future};
use futures_channel::oneshot;
use std::thread;

#[test]
fn ignore_err() {
    let v = vec![err(1), err(2), ok(3), ok(4)];

    let (i, v) = block_on(select_ok(v)).ok().unwrap();
    assert!(i == 3 || i == 4);

    assert!(v.len() < 4);

    let (j, v) = block_on(select_ok(v)).ok().unwrap();
    assert!(j == 3 || j == 4);
    assert_ne!(j, i);

    assert!(v.len() < 3);
}

#[test]
fn last_err() {
    let (ok_sender, ok_receiver) = oneshot::channel();
    let (first_err_sender, first_err_receiver) = oneshot::channel();
    let (second_err_sender, second_err_receiver) = oneshot::channel();
    async fn await_unwrap<T, E: Debug>(o: impl Future<Output = Result<T, E>>) -> T {
        o.await.unwrap()
    }
    let v = vec![
        Box::pin(await_unwrap(ok_receiver)),
        Box::pin(await_unwrap(first_err_receiver)),
        Box::pin(await_unwrap(second_err_receiver)),
    ];
    ok_sender.send(Ok(1)).unwrap();

    let (i, v) = block_on(select_ok(v)).ok().unwrap();
    assert_eq!(i, 1);

    assert_eq!(v.len(), 2);
    first_err_sender.send(Err(2)).unwrap();

    thread::spawn(move || {
        thread::sleep(Duration::from_millis(100));
        second_err_sender.send(Err(3)).unwrap();
    });

    let i = block_on(select_ok(v)).err().unwrap();
    assert_eq!(i, 3);
}

#[test]
fn is_fair() {
    let mut results = Vec::with_capacity(100);
    for _ in 0..100 {
        let v = vec![err(1), err(2), ok(3), ok(4)];

        let (i, _v) = block_on(select_ok(v)).ok().unwrap();
        results.push(i);
    }
    const THRESHOLD: usize = 30;
    assert_eq!(results.iter().filter(|i| **i == 3).take(THRESHOLD).count(), THRESHOLD);
    assert_eq!(results.iter().filter(|i| **i == 4).take(THRESHOLD).count(), THRESHOLD);
}

#[test]
fn is_biased() {
    let mut results = Vec::with_capacity(100);
    for _ in 0..100 {
        let v = vec![err(1), err(2), ok(3), ok(4)];

        let (i, _v) = block_on(select_ok_biased(v)).ok().unwrap();
        results.push(i);
    }
    assert!(results.iter().all(|i| *i == 3));
}
