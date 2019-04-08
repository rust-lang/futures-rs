use futures::executor::{block_on, block_on_stream};
use futures::Async::*;
use futures::future;
use futures::stream::FuturesUnordered;
use futures::channel::oneshot;
use std::panic::{self, AssertUnwindSafe};

mod support;

trait AssertSendSync: Send + Sync {}
impl AssertSendSync for FuturesUnordered<()> {}

#[test]
fn basic_usage() {
    block_on(future::lazy(move |cx| {
        let mut queue = FuturesUnordered::new();
        let (tx1, rx1) = oneshot::channel();
        let (tx2, rx2) = oneshot::channel();
        let (tx3, rx3) = oneshot::channel();

        queue.push(rx1);
        queue.push(rx2);
        queue.push(rx3);

        assert!(!queue.poll_next(cx).unwrap().is_ready());

        tx2.send("hello").unwrap();

        assert_eq!(Ready(Some("hello")), queue.poll_next(cx).unwrap());
        assert!(!queue.poll_next(cx).unwrap().is_ready());

        tx1.send("world").unwrap();
        tx3.send("world2").unwrap();

        assert_eq!(Ready(Some("world")), queue.poll_next(cx).unwrap());
        assert_eq!(Ready(Some("world2")), queue.poll_next(cx).unwrap());
        assert_eq!(Ready(None), queue.poll_next(cx).unwrap());

        Ok::<_, ()>(())
    })).unwrap();
}

#[test]
fn resolving_errors() {
    block_on(future::lazy(move |cx| {
        let mut queue = FuturesUnordered::new();
        let (tx1, rx1) = oneshot::channel();
        let (tx2, rx2) = oneshot::channel();
        let (tx3, rx3) = oneshot::channel();

        queue.push(rx1);
        queue.push(rx2);
        queue.push(rx3);

        assert!(!queue.poll_next(cx).unwrap().is_ready());

        drop(tx2);

        assert!(queue.poll_next(cx).is_err());
        assert!(!queue.poll_next(cx).unwrap().is_ready());

        drop(tx1);
        tx3.send("world2").unwrap();

        assert!(queue.poll_next(cx).is_err());
        assert_eq!(Ready(Some("world2")), queue.poll_next(cx).unwrap());
        assert_eq!(Ready(None), queue.poll_next(cx).unwrap());

        Ok::<_, ()>(())
    })).unwrap();
}

#[test]
fn dropping_ready_queue() {
    block_on(future::lazy(move |_| {
        let mut queue = FuturesUnordered::new();
        let (mut tx1, rx1) = oneshot::channel::<()>();
        let (mut tx2, rx2) = oneshot::channel::<()>();
        let (mut tx3, rx3) = oneshot::channel::<()>();

        queue.push(rx1);
        queue.push(rx2);
        queue.push(rx3);

        support::noop_waker_lw(|cx| {
            assert!(!tx1.poll_cancel(cx).unwrap().is_ready());
            assert!(!tx2.poll_cancel(cx).unwrap().is_ready());
            assert!(!tx3.poll_cancel(cx).unwrap().is_ready());

            drop(queue);

            assert!(tx1.poll_cancel(cx).unwrap().is_ready());
            assert!(tx2.poll_cancel(cx).unwrap().is_ready());
            assert!(tx3.poll_cancel(cx).unwrap().is_ready());
        });

        Ok::<_, ()>(()).into_future()
    })).unwrap();
}

#[test]
fn stress() {
    const ITER: usize = 300;

    use std::sync::{Arc, Barrier};
    use std::thread;

    for i in 0..ITER {
        let n = (i % 10) + 1;

        let mut queue = FuturesUnordered::new();

        for _ in 0..5 {
            let barrier = Arc::new(Barrier::new(n + 1));

            for num in 0..n {
                let barrier = barrier.clone();
                let (tx, rx) = oneshot::channel();

                queue.push(rx);

                thread::spawn(move || {
                    barrier.wait();
                    tx.send(num).unwrap();
                });
            }

            barrier.wait();

            let mut sync = block_on_stream(queue);

            let mut rx: Vec<_> = (&mut sync)
                .take(n)
                .map(|res| res.unwrap())
                .collect();

            assert_eq!(rx.len(), n);

            rx.sort();

            for num in 0..n {
                assert_eq!(rx[num], num);
            }

            queue = sync.into_inner();
        }
    }
}

#[test]
fn panicking_future_dropped() {
    block_on(future::lazy(move |cx| {
        let mut queue = FuturesUnordered::new();
        queue.push(future::poll_fn(|_| -> Poll<i32, i32> {
            panic!()
        }));

        let r = panic::catch_unwind(AssertUnwindSafe(|| queue.poll_next(cx)));
        assert!(r.is_err());
        assert!(queue.is_empty());
        assert_eq!(Ready(None), queue.poll_next(cx).unwrap());

        Ok::<_, ()>(())
    })).unwrap();
}
