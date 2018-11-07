#![feature(async_await, await_macro, pin, arbitrary_self_types, futures_api)]

use futures::future::{Future};
use futures_core::future::{ FusedFuture };
use futures::channel::manual_reset_event::{manual_reset_event, local_manual_reset_event};
use futures::executor::block_on;
use futures_test::task::{WakeCounter, panic_local_waker};
use pin_utils::pin_mut;
use std::thread;
use std::time;

#[test]
fn synchronous() {
    let event = manual_reset_event(false);

    assert!(!event.is_set());
    event.set();
    assert!(event.is_set());
    event.reset();
    assert!(!event.is_set());
}

#[test]
fn smoke() {
    let event = manual_reset_event(false);

    let waiters: Vec<thread::JoinHandle<time::Instant>> =
        [1..4].iter().map(|_| {
            let ev = event.clone();
            thread::spawn(move || {
                block_on(ev.poll_set());
                time::Instant::now()
            })
        }).collect();

    let start = time::Instant::now();

    thread::sleep(time::Duration::from_millis(100));
    event.set();

    for waiter in waiters.into_iter() {
        let end_time = waiter.join().unwrap();
        let diff = end_time - start;
        assert!(diff > time::Duration::from_millis(50));
    }
}

#[test]
fn immediately_ready_event() {
    let event = manual_reset_event(true);
    let lw = &panic_local_waker();

    assert!(event.is_set());

    let poll = event.poll_set();
    pin_mut!(poll);
    assert!(!poll.as_mut().is_terminated());

    assert!(poll.as_mut().poll(lw).is_ready());
    assert!(poll.as_mut().is_terminated());
}

#[test]
fn cancel_mid_wait() {
    let event = manual_reset_event(false);
    let wake_counter = WakeCounter::new();
    let lw = &wake_counter.local_waker();

    {
        // Cancel a wait in between other waits
        // In order to arbitrarily drop a non movable future we have to box and pin it
        let mut poll1 = Box::pinned(event.poll_set());
        let mut poll2 = Box::pinned(event.poll_set());
        let mut poll3 = Box::pinned(event.poll_set());
        let mut poll4 = Box::pinned(event.poll_set());
        let mut poll5 = Box::pinned(event.poll_set());

        assert!(poll1.as_mut().poll(lw).is_pending());
        assert!(poll2.as_mut().poll(lw).is_pending());
        assert!(poll3.as_mut().poll(lw).is_pending());
        assert!(poll4.as_mut().poll(lw).is_pending());
        assert!(poll5.as_mut().poll(lw).is_pending());
        assert!(!poll1.is_terminated());
        assert!(!poll2.is_terminated());
        assert!(!poll3.is_terminated());
        assert!(!poll4.is_terminated());
        assert!(!poll5.is_terminated());

        // Cancel 2 futures. Only the remaining ones should get completed
        drop(poll2);
        drop(poll4);

        assert!(poll1.as_mut().poll(lw).is_pending());
        assert!(poll3.as_mut().poll(lw).is_pending());
        assert!(poll5.as_mut().poll(lw).is_pending());

        assert_eq!(0, wake_counter.count());
        event.set();

        assert!(poll1.as_mut().poll(lw).is_ready());
        assert!(poll3.as_mut().poll(lw).is_ready());
        assert!(poll5.as_mut().poll(lw).is_ready());
        assert!(poll1.is_terminated());
        assert!(poll3.is_terminated());
        assert!(poll5.is_terminated());
    }

    assert_eq!(3, wake_counter.count());
}

#[test]
fn cancel_end_wait() {
    let event = manual_reset_event(false);
    let wake_counter = WakeCounter::new();
    let lw = &wake_counter.local_waker();

    let poll1 = event.poll_set();
    let poll2 = event.poll_set();
    let poll3 = event.poll_set();
    let poll4 = event.poll_set();

    pin_mut!(poll1);
    pin_mut!(poll2);
    pin_mut!(poll3);
    pin_mut!(poll4);

    assert!(poll1.as_mut().poll(lw).is_pending());
    assert!(poll2.as_mut().poll(lw).is_pending());

    // Start polling some wait handles which get cancelled
    // before new ones are attached
    {
        let poll5 = event.poll_set();
        let poll6 = event.poll_set();
        pin_mut!(poll5);
        pin_mut!(poll6);
        assert!(poll5.as_mut().poll(lw).is_pending());
        assert!(poll6.as_mut().poll(lw).is_pending());
    }

    assert!(poll3.as_mut().poll(lw).is_pending());
    assert!(poll4.as_mut().poll(lw).is_pending());

    event.set();

    assert!(poll1.as_mut().poll(lw).is_ready());
    assert!(poll2.as_mut().poll(lw).is_ready());
    assert!(poll3.as_mut().poll(lw).is_ready());
    assert!(poll4.as_mut().poll(lw).is_ready());

    assert_eq!(4, wake_counter.count());
}

#[test]
fn local_event() {
    let event = local_manual_reset_event(false);

    let wake_counter = WakeCounter::new();
    let lw = &wake_counter.local_waker();

    {
        // Cancel a wait in between other waits
        // In order to arbitrarily drop a non movable future we have to box and pin it
        let mut poll1 = Box::pinned(event.poll_set());
        let mut poll2 = Box::pinned(event.poll_set());
        let mut poll3 = Box::pinned(event.poll_set());
        let mut poll4 = Box::pinned(event.poll_set());
        let mut poll5 = Box::pinned(event.poll_set());

        assert!(poll1.as_mut().poll(lw).is_pending());
        assert!(poll2.as_mut().poll(lw).is_pending());
        assert!(poll3.as_mut().poll(lw).is_pending());
        assert!(poll4.as_mut().poll(lw).is_pending());
        assert!(poll5.as_mut().poll(lw).is_pending());
        assert!(!poll1.is_terminated());
        assert!(!poll2.is_terminated());
        assert!(!poll3.is_terminated());
        assert!(!poll4.is_terminated());
        assert!(!poll5.is_terminated());

        // Cancel 2 futures. Only the remaining ones should get completed
        drop(poll2);
        drop(poll4);

        assert!(poll1.as_mut().poll(lw).is_pending());
        assert!(poll3.as_mut().poll(lw).is_pending());
        assert!(poll5.as_mut().poll(lw).is_pending());

        assert_eq!(0, wake_counter.count());
        event.set();

        assert!(poll1.as_mut().poll(lw).is_ready());
        assert!(poll3.as_mut().poll(lw).is_ready());
        assert!(poll5.as_mut().poll(lw).is_ready());
        assert!(poll1.is_terminated());
        assert!(poll3.is_terminated());
        assert!(poll5.is_terminated());
    }

    assert_eq!(3, wake_counter.count());
}