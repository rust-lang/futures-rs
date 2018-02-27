#![feature(test)]

extern crate futures;
extern crate futures_util;
extern crate test;

use futures::prelude::*;
use futures::task::{self, Wake, Waker};
use futures::executor::LocalPool;
use futures_util::lock::BiLock;
use futures_util::lock::BiLockAcquire;
use futures_util::lock::BiLockAcquired;

use std::sync::Arc;
use test::Bencher;

fn notify_noop() -> Waker {
    struct Noop;

    impl Wake for Noop {
        fn wake(_: &Arc<Self>) {}
    }

    Waker::from(Arc::new(Noop))
}


/// Pseudo-stream which simply calls `lock.poll()` on `poll`
struct LockStream {
    lock: BiLockAcquire<u32>,
}

impl LockStream {
    fn new(lock: BiLock<u32>) -> LockStream {
        LockStream {
            lock: lock.lock()
        }
    }

    /// Release a lock after it was acquired in `poll`,
    /// so `poll` could be called again.
    fn release_lock(&mut self, guard: BiLockAcquired<u32>) {
        self.lock = guard.unlock().lock()
    }
}

impl Stream for LockStream {
    type Item = BiLockAcquired<u32>;
    type Error = ();

    fn poll_next(&mut self, cx: &mut task::Context) -> Poll<Option<Self::Item>, Self::Error> {
        self.lock.poll(cx).map(|a| match a {
            Async::Ready(a) => Async::Ready(Some(a)),
            Async::Pending => Async::Pending,
        })
    }
}


#[bench]
fn contended(b: &mut Bencher) {
    let pool = LocalPool::new();
    let mut exec = pool.executor();
    let waker = notify_noop();
    let mut map = task::LocalMap::new();
    let mut cx = task::Context::new(&mut map, &waker, &mut exec);

    b.iter(|| {
        let (x, y) = BiLock::new(1);

        let mut x = LockStream::new(x);
        let mut y = LockStream::new(y);

        for _ in 0..1000 {
            let x_guard = match x.poll_next(&mut cx) {
                Ok(Async::Ready(Some(guard))) => guard,
                _ => panic!(),
            };

            // Try poll second lock while first lock still holds the lock
            match y.poll_next(&mut cx) {
                Ok(Async::Pending) => (),
                _ => panic!(),
            };

            x.release_lock(x_guard);

            let y_guard = match y.poll_next(&mut cx) {
                Ok(Async::Ready(Some(guard))) => guard,
                _ => panic!(),
            };

            y.release_lock(y_guard);
        }
        (x, y)
    });
}

#[bench]
fn lock_unlock(b: &mut Bencher) {
    let pool = LocalPool::new();
    let mut exec = pool.executor();
    let waker = notify_noop();
    let mut map = task::LocalMap::new();
    let mut cx = task::Context::new(&mut map, &waker, &mut exec);

    b.iter(|| {
        let (x, y) = BiLock::new(1);

        let mut x = LockStream::new(x);
        let mut y = LockStream::new(y);

        for _ in 0..1000 {
            let x_guard = match x.poll_next(&mut cx) {
                Ok(Async::Ready(Some(guard))) => guard,
                _ => panic!(),
            };

            x.release_lock(x_guard);

            let y_guard = match y.poll_next(&mut cx) {
                Ok(Async::Ready(Some(guard))) => guard,
                _ => panic!(),
            };

            y.release_lock(y_guard);
        }
        (x, y)
    })
}
