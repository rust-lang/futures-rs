use futures::executor::block_on;
use futures::stream::{self, *};
use futures::task::*;
use std::convert::identity;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

struct DataStream {
    data: Vec<u8>,
    polled: bool,
    wake_immediately: bool,
    woken: Arc<AtomicBool>,
}

impl Stream for DataStream {
    type Item = u8;

    fn poll_next(mut self: Pin<&mut Self>, ctx: &mut Context) -> Poll<Option<Self::Item>> {
        if !self.polled {
            if !self.wake_immediately {
                let waker = ctx.waker().clone();
                let woken = self.woken.clone();
                let sleep_time = Duration::from_millis(*self.data.last().unwrap_or(&0) as u64);
                thread::spawn(move || {
                    thread::sleep(sleep_time);
                    woken.swap(true, Ordering::Relaxed);
                    waker.wake_by_ref();
                });
            } else {
                self.woken.swap(true, Ordering::Relaxed);
                ctx.waker().wake_by_ref();
            }
            self.polled = true;
            Poll::Pending
        } else {
            assert!(
                self.woken.swap(false, Ordering::AcqRel),
                "Inner stream polled before wake!"
            );
            self.polled = false;
            Poll::Ready(self.data.pop())
        }
    }
}

struct Interchanger {
    polled: bool,
    base: u8,
    wake_immediately: bool,
    woken: Arc<AtomicBool>,
}

impl Stream for Interchanger {
    type Item = DataStream;

    fn poll_next(mut self: Pin<&mut Self>, ctx: &mut Context) -> Poll<Option<Self::Item>> {
        if !self.polled {
            self.polled = true;
            if !self.wake_immediately {
                let waker = ctx.waker().clone();
                let woken = self.woken.clone();
                let sleep_time = Duration::from_millis(self.base as u64);
                thread::spawn(move || {
                    thread::sleep(sleep_time);
                    woken.swap(true, Ordering::Relaxed);
                    waker.wake_by_ref();
                });
            } else {
                self.woken.swap(true, Ordering::Relaxed);
                ctx.waker().wake_by_ref();
            }
            Poll::Pending
        } else {
            assert!(
                self.woken.swap(false, Ordering::AcqRel),
                "Stream polled before wake!"
            );
            self.base += 1;
            self.polled = false;
            Poll::Ready(Some(DataStream {
                polled: false,
                data: vec![9, 8, 7, 6, 5]
                    .into_iter()
                    .map(|v| v * self.base)
                    .collect(),
                wake_immediately: self.wake_immediately && self.base % 2 == 0,
                woken: Arc::new(AtomicBool::new(false)),
            }))
        }
    }
}

#[test]
fn select() {
    fn select_and_compare(a: Vec<u32>, b: Vec<u32>, expected: Vec<u32>) {
        let a = stream::iter(a);
        let b = stream::iter(b);
        let vec = block_on(stream::select(a, b).collect::<Vec<_>>());
        assert_eq!(vec, expected);
    }

    select_and_compare(vec![1, 2, 3], vec![4, 5, 6], vec![1, 4, 2, 5, 3, 6]);
    select_and_compare(vec![1, 2, 3], vec![4, 5], vec![1, 4, 2, 5, 3]);
    select_and_compare(vec![1, 2], vec![4, 5, 6], vec![1, 4, 2, 5, 6]);
}

#[test]
fn scan() {
    futures::executor::block_on(async {
        assert_eq!(
            stream::iter(vec![1u8, 2, 3, 4, 6, 8, 2])
                .scan(1, |state, e| {
                    *state += 1;
                    futures::future::ready(if e < *state { Some(e) } else { None })
                })
                .collect::<Vec<_>>()
                .await,
            vec![1u8, 2, 3, 4]
        );
    });
}

#[test]
fn flat_map_unordered() {
    futures::executor::block_on(async {
        let st = stream::iter(vec![
            stream::iter(0..=4u8),
            stream::iter(6..=10),
            stream::iter(0..=2),
        ]);

        let mut fm_unordered = st
            .flat_map_unordered(1, |s| s.filter(|v| futures::future::ready(v % 2 == 0)))
            .collect::<Vec<_>>()
            .await;

        fm_unordered.sort();

        assert_eq!(fm_unordered, vec![0, 0, 2, 2, 4, 6, 8, 10]);
    });
}

#[test]
fn flat_map_unordered_concurrency() {
    futures::executor::block_on(async {
        let fm_unordered = Interchanger {
            polled: false,
            base: 1,
            woken: Arc::new(AtomicBool::new(false)),
            wake_immediately: false,
        }
        .take(10)
        .flat_map_unordered(10, |s| s.map(identity))
        .collect::<Vec<_>>()
        .await;

        assert_eq!(fm_unordered.len(), 50);
    });
}

#[test]
fn flat_map_unordered_concurrency_when_wake_immediately() {
    futures::executor::block_on(async {
        let fm_unordered = Interchanger {
            polled: false,
            base: 1,
            woken: Arc::new(AtomicBool::new(false)),
            wake_immediately: true,
        }
        .take(10)
        .flat_map_unordered(10, |s| s.map(identity))
        .collect::<Vec<_>>()
        .await;

        assert_eq!(fm_unordered.len(), 50);
    });
}
