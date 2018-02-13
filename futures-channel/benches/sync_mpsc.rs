#![feature(test)]

extern crate futures;
extern crate futures_channel;
extern crate test;

use futures::task::{self, Notify, NotifyHandle};
use futures::prelude::*;

use futures_channel::mpsc::unbounded;
use futures_channel::mpsc::channel;
use futures_channel::mpsc::Sender;
use futures_channel::mpsc::UnboundedSender;


use test::Bencher;

fn notify_noop() -> NotifyHandle {
    struct Noop;

    impl Notify for Noop {
        fn notify(&self, _id: usize) {}
    }

    const NOOP : &'static Noop = &Noop;

    NotifyHandle::from(NOOP)
}

/// Single producer, single consumer
#[bench]
fn unbounded_1_tx(b: &mut Bencher) {
    b.iter(|| {
        let (tx, rx) = unbounded();

        let mut rx = task::spawn(rx);

        // 1000 iterations to avoid measuring overhead of initialization
        // Result should be divided by 1000
        for i in 0..1000 {

            // Poll, not ready, park
            assert_eq!(Ok(Async::Pending), rx.poll_stream_notify(&notify_noop(), 1));

            UnboundedSender::unbounded_send(&tx, i).unwrap();

            // Now poll ready
            assert_eq!(Ok(Async::Ready(Some(i))), rx.poll_stream_notify(&notify_noop(), 1));
        }
    })
}

/// 100 producers, single consumer
#[bench]
fn unbounded_100_tx(b: &mut Bencher) {
    b.iter(|| {
        let (tx, rx) = unbounded();

        let mut rx = task::spawn(rx);

        let tx: Vec<_> = (0..100).map(|_| tx.clone()).collect();

        // 1000 send/recv operations total, result should be divided by 1000
        for _ in 0..10 {
            for i in 0..tx.len() {
                assert_eq!(Ok(Async::Pending), rx.poll_stream_notify(&notify_noop(), 1));

                UnboundedSender::unbounded_send(&tx[i], i).unwrap();

                assert_eq!(Ok(Async::Ready(Some(i))), rx.poll_stream_notify(&notify_noop(), 1));
            }
        }
    })
}

#[bench]
fn unbounded_uncontended(b: &mut Bencher) {
    b.iter(|| {
        let (tx, mut rx) = unbounded();

        for i in 0..1000 {
            UnboundedSender::unbounded_send(&tx, i).expect("send");
            // No need to create a task, because poll is not going to park.
            assert_eq!(Ok(Async::Ready(Some(i))), rx.poll(&mut task::Context));
        }
    })
}


/// A Stream that continuously sends incrementing number of the queue
struct TestSender {
    tx: Sender<u32>,
    last: u32, // Last number sent
}

// Could be a Future, it doesn't matter
impl Stream for TestSender {
    type Item = u32;
    type Error = ();

    fn poll(&mut self, cx: &mut task::Context) -> Poll<Option<Self::Item>, Self::Error> {
        match self.tx.start_send(cx, self.last + 1) {
            Err(_) => panic!(),
            Ok(Ok(())) => {
                self.last += 1;
                assert_eq!(Ok(Async::Ready(())), self.tx.flush(&mut task::Context));
                Ok(Async::Ready(Some(self.last)))
            }
            Ok(Err(_)) => {
                Ok(Async::Pending)
            }
        }
    }
}


/// Single producers, single consumer
#[bench]
fn bounded_1_tx(b: &mut Bencher) {
    b.iter(|| {
        let (tx, rx) = channel(0);

        let mut tx = task::spawn(TestSender {
            tx: tx,
            last: 0,
        });

        let mut rx = task::spawn(rx);

        for i in 0..1000 {
            assert_eq!(Ok(Async::Ready(Some(i + 1))), tx.poll_stream_notify(&notify_noop(), 1));
            assert_eq!(Ok(Async::Pending), tx.poll_stream_notify(&notify_noop(), 1));
            assert_eq!(Ok(Async::Ready(Some(i + 1))), rx.poll_stream_notify(&notify_noop(), 1));
        }
    })
}

/// 100 producers, single consumer
#[bench]
fn bounded_100_tx(b: &mut Bencher) {
    b.iter(|| {
        // Each sender can send one item after specified capacity
        let (tx, rx) = channel(0);

        let mut tx: Vec<_> = (0..100).map(|_| {
            task::spawn(TestSender {
                tx: tx.clone(),
                last: 0
            })
        }).collect();

        let mut rx = task::spawn(rx);

        for i in 0..10 {
            for j in 0..tx.len() {
                // Send an item
                assert_eq!(Ok(Async::Ready(Some(i + 1))), tx[j].poll_stream_notify(&notify_noop(), 1));
                // Then block
                assert_eq!(Ok(Async::Pending), tx[j].poll_stream_notify(&notify_noop(), 1));
                // Recv the item
                assert_eq!(Ok(Async::Ready(Some(i + 1))), rx.poll_stream_notify(&notify_noop(), 1));
            }
        }
    })
}
