#![cfg(feature = "use_std")]

extern crate futures;

use futures::prelude::*;
use futures::future::{lazy, ok};
use futures::stream::unfold;
use futures::sync::slot;
use futures::sync::mpsc;

use std::time::Duration;
use std::thread;
use std::sync::Arc;

mod support;
use support::*;


trait AssertSend: Send {}
impl AssertSend for slot::Sender<i32> {}
impl AssertSend for slot::Receiver<i32> {}

#[test]
fn send_recv() {
    let (tx, rx) = slot::channel::<i32>();
    let mut rx = rx.wait();

    tx.send(1).wait().unwrap();

    assert_eq!(rx.next().unwrap(), Ok(1));
}

#[test]
fn send_error() {
    let (tx, rx) = slot::channel::<i32>();
    drop(rx);

    assert_eq!(tx.swap(1).unwrap_err().into_inner(), 1);
}

#[test]
fn swap() {
    let (tx, rx) = slot::channel::<i32>();
    let mut rx = rx.wait();

    assert_eq!(tx.swap(1), Ok(None));
    assert_eq!(tx.swap(2), Ok(Some(1)));
    assert_eq!(rx.next().unwrap(), Ok(2));
    assert_eq!(tx.swap(3), Ok(None));
    assert_eq!(rx.next().unwrap(), Ok(3));
}

#[test]
fn send_recv_no_buffer() {
    let (mut tx, mut rx) = slot::channel::<i32>();

    // Run on a task context
    lazy(move || {
        assert!(tx.poll_complete().unwrap().is_ready());

        // Send first message

        let res = tx.start_send(1).unwrap();
        assert!(is_ready(&res));

        // Send second message
        let res = tx.start_send(2).unwrap();
        assert!(is_ready(&res));

        // Take the value
        assert_eq!(rx.poll().unwrap(), Async::Ready(Some(2)));

        let res = tx.start_send(3).unwrap();
        assert!(is_ready(&res));

        // Take the value
        assert_eq!(rx.poll().unwrap(), Async::Ready(Some(3)));

        Ok::<(), ()>(())
    }).wait().unwrap();
}

#[test]
fn send_shared_recv() {
    let (tx, rx) = slot::channel::<i32>();
    let tx1 = Arc::new(tx);
    let tx2 = tx1.clone();
    let mut rx = rx.wait();

    tx1.swap(1).unwrap();
    assert_eq!(rx.next().unwrap(), Ok(1));

    tx2.swap(2).unwrap();
    assert_eq!(rx.next().unwrap(), Ok(2));
}

#[test]
fn send_recv_threads() {
    let (tx, rx) = slot::channel::<i32>();
    let mut rx = rx.wait();

    thread::spawn(move|| {
        tx.send(1).wait().unwrap();
    });

    assert_eq!(rx.next().unwrap(), Ok(1));
}

#[test]
fn send_recv_threads_no_capacity() {
    let (mut tx, rx) = slot::channel::<i32>();
    let mut rx = rx.wait();

    let t = thread::spawn(move|| {
        tx = tx.send(1).wait().unwrap();
        tx = tx.send(2).wait().unwrap();
    });

    thread::sleep(Duration::from_millis(100));
    assert_eq!(rx.next().unwrap(), Ok(2));

    t.join().unwrap();
}

#[test]
fn tx_close_gets_none() {
    let (_, mut rx) = slot::channel::<i32>();

    // Run on a task context
    lazy(move || {
        assert_eq!(rx.poll(), Ok(Async::Ready(None)));
        assert_eq!(rx.poll(), Ok(Async::Ready(None)));

        Ok::<(), ()>(())
    }).wait().unwrap();
}

#[test]
fn spawn_sends_items() {
    let core = local_executor::Core::new();
    let stream = unfold(0, |i| Some(ok::<_,u8>((i, i + 1))));
    let rx = mpsc::spawn(stream, &core, 1);
    assert_eq!(core.run(rx.take(4).collect()).unwrap(),
               [0, 1, 2, 3]);
}

#[test]
fn stress_shared_bounded_hard() {
    const AMT: u32 = 10000;
    const NTHREADS: u32 = 8;
    let (tx, rx) = slot::channel::<i32>();
    let mut rx = rx.wait();

    let t = thread::spawn(move|| {
        for _ in 0..AMT * NTHREADS {
            if let Some(value) = rx.next() {
                assert_eq!(value, Ok(1));
            } else {
                // less items is okay, but it must be end of stream
                break;
            }
        }

        if rx.next().is_some() {
            panic!();
        }
    });
    let tx = Arc::new(tx);

    for _ in 0..NTHREADS {
        let tx = tx.clone();

        thread::spawn(move|| {
            for _ in 0..AMT {
                tx.swap(1).unwrap();
            }
        });
    }

    drop(tx);

    t.join().ok().unwrap();
}

/// Stress test that receiver properly receivesl last message
/// after sender dropped.
#[test]
fn stress_drop_sender() {
    fn list() -> Box<Stream<Item=i32, Error=u32>> {
        let (tx, rx) = slot::channel();
        tx.send(Ok(1))
          .and_then(|tx| tx.send(Ok(2)))
          .and_then(|tx| tx.send(Ok(3)))
          .forget();
        Box::new(rx.then(|r| r.unwrap()))
    }

    for _ in 0..10000 {
        assert_eq!(
            list().wait().collect::<Result<Vec<_>, _>>().unwrap().last(),
            Some(&3));
    }
}

fn is_ready<T>(res: &AsyncSink<T>) -> bool {
    match *res {
        AsyncSink::Ready => true,
        _ => false,
    }
}
