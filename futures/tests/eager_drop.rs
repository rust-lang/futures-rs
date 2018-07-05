#![feature(pin, arbitrary_self_types, futures_api)]

#[macro_use]
extern crate futures;

use futures::channel::oneshot;
use futures::future;
use futures::prelude::*;
use std::mem::PinMut;
use std::sync::mpsc;

mod support;
use self::support::RunInBackgroundExt;

#[test]
fn map_ok() {
    // The closure given to `map_ok` should have been dropped by the time `map`
    // runs.
    let (tx1, rx1) = mpsc::channel::<()>();
    let (tx2, rx2) = mpsc::channel::<()>();

    future::ready::<Result<i32, i32>>(Err(1))
        .map_ok(move |_| { let _tx1 = tx1; panic!("should not run"); })
        .map(move |_| {
            assert!(rx1.recv().is_err());
            tx2.send(()).unwrap()
        })
        .run_in_background();

    rx2.recv().unwrap();
}

#[test]
fn map_err() {
    // The closure given to `map_err` should have been dropped by the time `map`
    // runs.
    let (tx1, rx1) = mpsc::channel::<()>();
    let (tx2, rx2) = mpsc::channel::<()>();

    future::ready::<Result<i32, i32>>(Ok(1))
        .map_err(move |_| { let _tx1 = tx1; panic!("should not run"); })
        .map(move |_| {
            assert!(rx1.recv().is_err());
            tx2.send(()).unwrap()
        })
        .run_in_background();

    rx2.recv().unwrap();
}

struct FutureData<F, T> {
    _data: T,
    future: F,
}

impl<F, T> FutureData<F, T> {
    unsafe_pinned!(future -> F);
}

impl<F: Future, T: Send + 'static> Future for FutureData<F, T> {
    type Output = F::Output;

    fn poll(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<F::Output> {
        self.future().poll(cx)
    }
}

#[test]
fn then_drops_eagerly() {
    let (start_tx, start_rx) = oneshot::channel::<()>();
    let (tx1, rx1) = mpsc::channel::<()>();
    let (tx2, rx2) = mpsc::channel::<()>();

    FutureData { _data: tx1, future: start_rx.unwrap_or_else(|_| { panic!() }) }
        .then(move |_| {
            assert!(rx1.recv().is_err()); // tx1 should have been dropped
            tx2.send(()).unwrap();
            future::ready(())
        })
        .run_in_background();

    assert_eq!(Err(mpsc::TryRecvError::Empty), rx2.try_recv());
    start_tx.send(()).unwrap();
    rx2.recv().unwrap();
}

#[test]
fn and_then_drops_eagerly() {
    let (start_tx, start_rx) = oneshot::channel::<Result<(), ()>>();
    let (tx1, rx1) = mpsc::channel::<()>();
    let (tx2, rx2) = mpsc::channel::<()>();

    FutureData { _data: tx1, future: start_rx.unwrap_or_else(|_| { panic!() }) }
        .and_then(move |_| {
            assert!(rx1.recv().is_err()); // tx1 should have been dropped
            tx2.send(()).unwrap();
            future::ready(Ok(()))
        })
        .run_in_background();

    assert_eq!(Err(mpsc::TryRecvError::Empty), rx2.try_recv());
    start_tx.send(Ok(())).unwrap();
    rx2.recv().unwrap();
}

#[test]
fn or_else_drops_eagerly() {
    let (start_tx, start_rx) = oneshot::channel::<Result<(), ()>>();
    let (tx1, rx1) = mpsc::channel::<()>();
    let (tx2, rx2) = mpsc::channel::<()>();

    FutureData { _data: tx1, future: start_rx.unwrap_or_else(|_| { panic!() }) }
        .or_else(move |_| {
            assert!(rx1.recv().is_err()); // tx1 should have been dropped
            tx2.send(()).unwrap();
            future::ready::<Result<(), ()>>(Ok(()))
        })
        .run_in_background();

    assert_eq!(Err(mpsc::TryRecvError::Empty), rx2.try_recv());
    start_tx.send(Err(())).unwrap();
    rx2.recv().unwrap();
}
