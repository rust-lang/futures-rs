#![feature(test)]

extern crate futures;
extern crate futures_channel;
extern crate test;

use futures::task::{self, Wake, Waker};
use futures::executor::LocalPool;
use futures::prelude::*;

use futures_channel::mpsc::unbounded;
use futures_channel::mpsc::channel;
use futures_channel::mpsc::Sender;
use futures_channel::mpsc::UnboundedSender;

use test::Bencher;

fn notify_noop() -> Waker {
    struct Noop;

    impl Wake for Noop {
        fn wake(&self) {}
    }

    const NOOP : &'static Noop = &Noop;

    Waker::from(NOOP)
}

/// Single producer, single consumer
#[bench]
fn unbounded_1_tx(b: &mut Bencher) {
    b.iter(|| {
        let (tx, mut rx) = unbounded();
        let pool = LocalPool::new();
        let exec = pool.executor();
        let waker = notify_noop();
        let mut map = task::LocalMap::new();
        let mut cx = task::Context::new(&mut map, &waker, &exec);

        // 1000 iterations to avoid measuring overhead of initialization
        // Result should be divided by 1000
        for i in 0..1000 {

            // Poll, not ready, park
            assert_eq!(Ok(Async::Pending), rx.poll(&mut cx));

            UnboundedSender::unbounded_send(&tx, i).unwrap();

            // Now poll ready
            assert_eq!(Ok(Async::Ready(Some(i))), rx.poll(&mut cx));
        }
    })
}

/// 100 producers, single consumer
#[bench]
fn unbounded_100_tx(b: &mut Bencher) {
    b.iter(|| {
        let (tx, mut rx) = unbounded();
        let pool = LocalPool::new();
        let exec = pool.executor();
        let waker = notify_noop();
        let mut map = task::LocalMap::new();
        let mut cx = task::Context::new(&mut map, &waker, &exec);

        let tx: Vec<_> = (0..100).map(|_| tx.clone()).collect();

        // 1000 send/recv operations total, result should be divided by 1000
        for _ in 0..10 {
            for i in 0..tx.len() {
                assert_eq!(Ok(Async::Pending), rx.poll(&mut cx));

                UnboundedSender::unbounded_send(&tx[i], i).unwrap();

                assert_eq!(Ok(Async::Ready(Some(i))), rx.poll(&mut cx));
            }
        }
    })
}

#[bench]
fn unbounded_uncontended(b: &mut Bencher) {
    let pool = LocalPool::new();
    let exec = pool.executor();
    let waker = notify_noop();
    let mut map = task::LocalMap::new();
    let mut cx = task::Context::new(&mut map, &waker, &exec);

    b.iter(|| {
        let (tx, mut rx) = unbounded();

        for i in 0..1000 {
            UnboundedSender::unbounded_send(&tx, i).expect("send");
            // No need to create a task, because poll is not going to park.
            assert_eq!(Ok(Async::Ready(Some(i))), rx.poll(&mut cx));
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
                assert_eq!(Ok(Async::Ready(())), self.tx.flush(cx));
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
    let pool = LocalPool::new();
    let exec = pool.executor();
    let waker = notify_noop();
    let mut map = task::LocalMap::new();
    let mut cx = task::Context::new(&mut map, &waker, &exec);

    b.iter(|| {
        let (tx, mut rx) = channel(0);

        let mut tx = TestSender {
            tx: tx,
            last: 0,
        };

        for i in 0..1000 {
            assert_eq!(Ok(Async::Ready(Some(i + 1))), tx.poll(&mut cx));
            assert_eq!(Ok(Async::Pending), tx.poll(&mut cx));
            assert_eq!(Ok(Async::Ready(Some(i + 1))), rx.poll(&mut cx));
        }
    })
}

/// 100 producers, single consumer
#[bench]
fn bounded_100_tx(b: &mut Bencher) {
    b.iter(|| {
        // Each sender can send one item after specified capacity
        let (tx, mut rx) = channel(0);
        let pool = LocalPool::new();
        let exec = pool.executor();
        let waker = notify_noop();
        let mut map = task::LocalMap::new();
        let mut cx = task::Context::new(&mut map, &waker, &exec);

        let mut tx: Vec<_> = (0..100).map(|_| {
            TestSender {
                tx: tx.clone(),
                last: 0
            }
        }).collect();

        for i in 0..10 {
            for j in 0..tx.len() {
                // Send an item
                assert_eq!(Ok(Async::Ready(Some(i + 1))), tx[j].poll(&mut cx));
                // Then block
                assert_eq!(Ok(Async::Pending), tx[j].poll(&mut cx));
                // Recv the item
                assert_eq!(Ok(Async::Ready(Some(i + 1))), rx.poll(&mut cx));
            }
        }
    })
}
