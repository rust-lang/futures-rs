#![feature(test)]

extern crate futures;
extern crate test;

use futures::{Async, Poll};
use futures::executor;
use futures::executor::{Notify, NotifyHandle};
use futures::sync::BiLock;
use futures::sync::BiLockAcquire;
use futures::sync::BiLockAcquired;
use futures::future::Future;
use futures::stream::Stream;


use test::Bencher;

fn notify_noop() -> NotifyHandle {
    struct Noop;

    impl Notify for Noop {
        fn notify(&self, _id: usize) {}
    }

    const NOOP : &'static Noop = &Noop;

    NotifyHandle::from(NOOP)
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

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        self.lock.poll().map(|a| match a {
            Async::Ready(a) => Async::Ready(Some(a)),
            Async::NotReady => Async::NotReady,
        })
    }
}


#[bench]
fn contended(b: &mut Bencher) {
    b.iter(|| {
        let (x, y) = BiLock::new(1);

        let mut x = executor::spawn(LockStream::new(x));
        let mut y = executor::spawn(LockStream::new(y));

        for _ in 0..1000 {
            let x_guard = match x.poll_stream_notify(&notify_noop(), 11) {
                Ok(Async::Ready(Some(guard))) => guard,
                _ => panic!(),
            };

            // Try poll second lock while first lock still holds the lock
            match y.poll_stream_notify(&notify_noop(), 11) {
                Ok(Async::NotReady) => (),
                _ => panic!(),
            };

            x.get_mut().release_lock(x_guard);

            let y_guard = match y.poll_stream_notify(&notify_noop(), 11) {
                Ok(Async::Ready(Some(guard))) => guard,
                _ => panic!(),
            };

            y.get_mut().release_lock(y_guard);
        }
        (x, y)
    });
}

#[bench]
fn lock_unlock(b: &mut Bencher) {
    b.iter(|| {
        let (x, y) = BiLock::new(1);

        let mut x = executor::spawn(LockStream::new(x));
        let mut y = executor::spawn(LockStream::new(y));

        for _ in 0..1000 {
            let x_guard = match x.poll_stream_notify(&notify_noop(), 11) {
                Ok(Async::Ready(Some(guard))) => guard,
                _ => panic!(),
            };

            x.get_mut().release_lock(x_guard);

            let y_guard = match y.poll_stream_notify(&notify_noop(), 11) {
                Ok(Async::Ready(Some(guard))) => guard,
                _ => panic!(),
            };

            y.get_mut().release_lock(y_guard);
        }
        (x, y)
    })
}

#[bench]
fn concurrent(b: &mut Bencher) {
    use std::thread;

    b.iter(|| {
        let (mut x, mut y) = BiLock::new(false);
        const ITERATION_COUNT: usize = 1000;

        let a = thread::spawn(move || {
            let mut count = 0;
            while count < ITERATION_COUNT {
                let mut guard = x.lock().wait().unwrap();
                if *guard {
                    *guard = false;
                    count += 1;
                }
                x = guard.unlock();
            }
        });

        let b = thread::spawn(move || {
            let mut count = 0;
            while count < ITERATION_COUNT {
                let mut guard = y.lock().wait().unwrap();
                if !*guard {
                    *guard = true;
                    count += 1;
                }
                y = guard.unlock();
            }
        });

        a.join().unwrap();
        b.join().unwrap();
    })
}
