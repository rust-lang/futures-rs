#![feature(test)]

extern crate futures;
extern crate test;

use futures::Async;
use futures::executor;
use futures::executor::{Notify, NotifyHandle};
use futures::sync::BiLock;


use test::Bencher;

fn notify_noop() -> NotifyHandle {
    struct Noop;

    impl Notify for Noop {
        fn notify(&self, _id: usize) {}
    }

    const NOOP : &'static Noop = &Noop;

    NotifyHandle::from(NOOP)
}

#[bench]
fn contended(b: &mut Bencher) {
    b.iter(|| {
        let mut t = BiLock::new(1);
        for _ in 0..1000 {
            let (x, y) = t;
            let x_lock = match executor::spawn(x.lock()).poll_future_notify(&notify_noop(), 11).unwrap() {
                Async::Ready(lock) => lock,
                Async::NotReady => panic!(),
            };

            // Try poll second lock while first lock still holds the lock
            let mut y = executor::spawn(y.lock());
            match y.poll_future_notify(&notify_noop(), 11).unwrap() {
                Async::Ready(_) => panic!(),
                Async::NotReady => (),
            };

            let x = x_lock.unlock();

            let y_lock = match y.poll_future_notify(&notify_noop(), 11).unwrap() {
                Async::Ready(lock) => lock,
                Async::NotReady => panic!(),
            };

            let y = y_lock.unlock();
            t = (x, y);
        }
        t
    });
}

#[bench]
fn lock_unlock(b: &mut Bencher) {
    b.iter(|| {
        let mut t = BiLock::new(1);
        for _ in 0..1000 {
            let (x, y) = t;
            let x_lock = match executor::spawn(x.lock()).poll_future_notify(&notify_noop(), 11).unwrap() {
                Async::Ready(lock) => lock,
                Async::NotReady => panic!(),
            };

            let x = x_lock.unlock();

            let mut y = executor::spawn(y.lock());
            let y_lock = match y.poll_future_notify(&notify_noop(), 11).unwrap() {
                Async::Ready(lock) => lock,
                Async::NotReady => panic!(),
            };

            let y = y_lock.unlock();
            t = (x, y);
        }
        t
    })
}
