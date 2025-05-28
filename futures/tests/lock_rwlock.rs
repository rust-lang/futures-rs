#![allow(dead_code)]
use futures::channel::mpsc;
use futures::executor::{block_on, ThreadPool};
use futures::future::{ready, FutureExt};
use futures::lock::RwLock;
use futures::stream::StreamExt;
use futures::task::{Context, SpawnExt};
use futures_test::future::FutureTestExt;
use futures_test::task::{new_count_waker, noop_context, panic_context};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

#[test]
fn rwlock_read_acquire_uncontested() {
    let rwlock = RwLock::new(());
    let lock = rwlock.read().poll_unpin(&mut panic_context());
    assert!(lock.is_ready());

    for _ in 0..10 {
        assert!(rwlock.read().poll_unpin(&mut panic_context()).is_ready());
    }
}

#[test]
fn rwlock_write_acquire_uncontested() {
    let rwlock = RwLock::new(());
    for _ in 0..10 {
        assert!(rwlock.write().poll_unpin(&mut panic_context()).is_ready());
    }
}

#[test]
fn rwlock_write_blocks_readers() {
    let rwlock = RwLock::new(());
    let lock = rwlock.write().poll_unpin(&mut panic_context());
    assert!(lock.is_ready());

    let read = rwlock.read().poll_unpin(&mut noop_context());
    assert!(read.is_pending());
}

#[test]
fn rwlock_write_blocks_writers() {
    let rwlock = RwLock::new(());
    let lock = rwlock.write().poll_unpin(&mut panic_context());
    assert!(lock.is_ready());

    let write = rwlock.write().poll_unpin(&mut noop_context());
    assert!(write.is_pending());
}

#[test]
fn rwlock_read_blocks_writers() {
    let rwlock = RwLock::new(());
    let lock = rwlock.read().poll_unpin(&mut panic_context());
    assert!(lock.is_ready());

    let write = rwlock.write().poll_unpin(&mut noop_context());
    assert!(write.is_pending());
}

#[test]
fn rwlock_read_wakes_write_waiters() {
    let rwlock = RwLock::new(());
    let (waker, counter) = new_count_waker();
    let lock = rwlock.read().poll_unpin(&mut panic_context());
    assert!(lock.is_ready());

    let mut cx = Context::from_waker(&waker);
    let mut waiter = rwlock.write();
    assert!(waiter.poll_unpin(&mut cx).is_pending());
    assert_eq!(counter, 0);

    drop(lock);

    assert_eq!(counter, 1);
    assert!(waiter.poll_unpin(&mut panic_context()).is_ready());
}

#[test]
fn rwlock_write_wakes_read_waiters() {
    let rwlock = RwLock::new(());
    let (waker, counter) = new_count_waker();
    let lock = rwlock.write().poll_unpin(&mut panic_context());
    assert!(lock.is_ready());

    let mut cx = Context::from_waker(&waker);
    let mut waiter = rwlock.read();
    assert!(waiter.poll_unpin(&mut cx).is_pending());
    assert_eq!(counter, 0);

    drop(lock);

    assert_eq!(counter, 1);
    assert!(waiter.poll_unpin(&mut panic_context()).is_ready());
}

#[test]
fn rwlock_write_wakes_write_waiters() {
    let rwlock = RwLock::new(());
    let (waker, counter) = new_count_waker();
    let lock = rwlock.write().poll_unpin(&mut panic_context());
    assert!(lock.is_ready());

    let mut cx = Context::from_waker(&waker);
    let mut waiter = rwlock.write();
    assert!(waiter.poll_unpin(&mut cx).is_pending());
    assert_eq!(counter, 0);

    drop(lock);

    assert_eq!(counter, 1);
    assert!(waiter.poll_unpin(&mut panic_context()).is_ready());
}

#[test]
fn rwlock_concurrent_reads() {
    let (tx, mut rx) = mpsc::unbounded();
    let pool = ThreadPool::builder().pool_size(16).create().unwrap();

    let tx = Arc::new(tx);
    let rwlock = Arc::new(RwLock::new(AtomicU32::new(0)));

    let num_tasks = 1000;
    for _ in 0..num_tasks {
        let tx = tx.clone();
        let rwlock = rwlock.clone();
        pool.spawn(async move {
            let lock = rwlock.read().await;
            ready(()).pending_once().await;
            lock.fetch_add(1, Ordering::Relaxed);
            tx.unbounded_send(()).unwrap();
            drop(lock);
        })
        .unwrap();
    }

    block_on(async {
        for _ in 0..num_tasks {
            rx.next().await.unwrap();
        }
        let lock = rwlock.read().await;
        assert_eq!(num_tasks, lock.load(Ordering::SeqCst));
    })
}

#[test]
fn rwlock_concurrent_writes() {
    let (tx, mut rx) = mpsc::unbounded();
    let pool = ThreadPool::builder().pool_size(16).create().unwrap();

    let tx = Arc::new(tx);
    let rwlock = Arc::new(RwLock::new(0));

    let num_tasks = 1000;
    for _ in 0..num_tasks {
        let tx = tx.clone();
        let rwlock = rwlock.clone();
        pool.spawn(async move {
            let mut lock = rwlock.write().await;
            ready(()).pending_once().await;
            *lock += 1;
            tx.unbounded_send(()).unwrap();
            drop(lock);
        })
        .unwrap();
    }

    block_on(async {
        for _ in 0..num_tasks {
            rx.next().await.unwrap();
        }
        let lock = rwlock.read().await;
        assert_eq!(num_tasks, *lock);
    })
}

#[test]
fn rwlock_concurrent_reads_and_writes() {
    let (tx, mut rx) = mpsc::unbounded();
    let pool = ThreadPool::builder().pool_size(16).create().unwrap();

    let tx = Arc::new(tx);
    let rwlock = Arc::new(RwLock::new(AtomicU32::new(0)));

    let num_tasks = 1000;
    for task in 0..num_tasks {
        let tx = tx.clone();
        let rwlock = rwlock.clone();
        pool.spawn(async move {
            if task & 1 == 0 {
                let lock = rwlock.read().await;
                ready(()).pending_once().await;
                lock.fetch_add(1, Ordering::Relaxed);
                tx.unbounded_send(()).unwrap();
                drop(lock);
            } else {
                let lock = rwlock.write().await;
                ready(()).pending_once().await;
                lock.fetch_add(1, Ordering::Relaxed);
                tx.unbounded_send(()).unwrap();
                drop(lock);
            }
        })
        .unwrap();
    }

    block_on(async {
        for _ in 0..num_tasks {
            rx.next().await.unwrap();
        }
        let lock = rwlock.read().await;
        assert_eq!(num_tasks, lock.load(Ordering::SeqCst));
    })
}

#[test]
fn rwlock_try_read_uncontested() {
    let rwlock = RwLock::new(());
    let lock = rwlock.try_read();
    assert!(lock.is_some());

    for _ in 0..10 {
        assert!(rwlock.try_read().is_some());
    }
}

#[test]
fn rwlock_try_write_uncontested() {
    let rwlock = RwLock::new(());
    for _ in 0..10 {
        assert!(rwlock.try_write().is_some());
    }
}

#[test]
fn rwlock_write_blocks_try_read() {
    let rwlock = RwLock::new(());
    let lock = rwlock.try_write();
    assert!(lock.is_some());
    assert!(rwlock.try_read().is_none());
    drop(lock);
    assert!(rwlock.try_read().is_some());
}

#[test]
fn rwlock_write_blocks_try_write() {
    let rwlock = RwLock::new(());
    let lock = rwlock.try_write();
    assert!(lock.is_some());
    assert!(rwlock.try_write().is_none());
    drop(lock);
    assert!(rwlock.try_write().is_some());
}

#[test]
fn rwlock_read_blocks_try_write() {
    let rwlock = RwLock::new(());
    let lock = rwlock.try_read();
    assert!(lock.is_some());
    assert!(rwlock.try_write().is_none());
    drop(lock);
    assert!(rwlock.try_write().is_some());
}
