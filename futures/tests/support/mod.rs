#![allow(dead_code)]

use std::fmt::{self, Debug};
use std::sync::Arc;
use std::thread;

use futures::executor::{block_on, Executor, SpawnError};
use futures::{Future, IntoFuture, Async, Poll, Never};
use futures::future::FutureResult;
use futures::stream::Stream;
use futures::task::{self, Waker, Wake};

pub fn f_ok(a: i32) -> FutureResult<i32, u32> { Ok(a).into_future() }
pub fn f_err(a: u32) -> FutureResult<i32, u32> { Err(a).into_future() }
pub fn r_ok(a: i32) -> Result<i32, u32> { Ok(a) }
pub fn r_err(a: u32) -> Result<i32, u32> { Err(a) }

pub fn assert_done<T, F>(f: F, result: Result<T::Item, T::Error>)
    where T: Future,
          T::Item: Eq + fmt::Debug,
          T::Error: Eq + fmt::Debug,
          F: FnOnce() -> T,
{
    assert_eq!(block_on(f()), result);
}

pub fn assert_empty<T: Future, F: FnMut() -> T>(mut f: F)
    where T::Error: Debug
{
    panic_waker_cx(|cx| {
        assert!(f().poll(cx).unwrap().is_pending())
    })
}

pub fn sassert_done<S: Stream>(s: &mut S) {
    panic_waker_cx(|cx| {
        match s.poll_next(cx) {
            Ok(Async::Ready(None)) => {}
            Ok(Async::Ready(Some(_))) => panic!("stream had more elements"),
            Ok(Async::Pending) => panic!("stream wasn't ready"),
            Err(_) => panic!("stream had an error"),
        }
    })
}

pub fn sassert_empty<S: Stream>(s: &mut S) {
    noop_waker_cx(|cx| {
        match s.poll_next(cx) {
            Ok(Async::Ready(None)) => panic!("stream is at its end"),
            Ok(Async::Ready(Some(_))) => panic!("stream had more elements"),
            Ok(Async::Pending) => {}
            Err(_) => panic!("stream had an error"),
        }
    })
}

pub fn sassert_next<S: Stream>(s: &mut S, item: S::Item)
    where S::Item: Eq + fmt::Debug
{
    panic_waker_cx(|cx| {
        match s.poll_next(cx) {
            Ok(Async::Ready(None)) => panic!("stream is at its end"),
            Ok(Async::Ready(Some(e))) => assert_eq!(e, item),
            Ok(Async::Pending) => panic!("stream wasn't ready"),
            Err(_) => panic!("stream had an error"),
        }
    })
}

pub fn sassert_err<S: Stream>(s: &mut S, err: S::Error)
    where S::Error: Eq + fmt::Debug
{
    panic_waker_cx(|cx| {
        match s.poll_next(cx) {
            Ok(Async::Ready(None)) => panic!("stream is at its end"),
            Ok(Async::Ready(Some(_))) => panic!("stream had more elements"),
            Ok(Async::Pending) => panic!("stream wasn't ready"),
            Err(e) => assert_eq!(e, err),
        }
    })
}

pub fn panic_waker_cx<F, R>(f: F) -> R
    where F: FnOnce(&mut task::Context) -> R
{
    struct Foo;

    impl Wake for Foo {
        fn wake(_: &Arc<Self>) {
            panic!("should not be woken");
        }
    }

    let map = &mut task::LocalMap::new();
    let panic_waker = Waker::from(Arc::new(Foo));
    let exec = &mut PanicExec;

    let cx = &mut task::Context::new(map, &panic_waker, exec);
    f(cx)
}

pub fn noop_waker_cx<F, R>(f: F) -> R 
    where F: FnOnce(&mut task::Context) -> R
{
    struct Noop;

    impl Wake for Noop {
        fn wake(_: &Arc<Self>) {}
    }

    let map = &mut task::LocalMap::new();
    let noop_waker = Waker::from(Arc::new(Noop));
    let exec = &mut PanicExec;

    let cx = &mut task::Context::new(map, &noop_waker, exec);
    f(cx)
}

pub struct PanicExec;
impl Executor for PanicExec {
    fn spawn(&mut self, _: Box<Future<Item = (), Error = Never> + Send>)
        -> Result<(), SpawnError>
    {
        panic!("should not spawn")
    }
}

pub trait ForgetExt {
    fn forget(self);
}

impl<F> ForgetExt for F
    where F: Future + Sized + Send + 'static,
          F::Item: Send,
          F::Error: Send
{
    fn forget(self) {
        thread::spawn(|| block_on(self));
    }
}

pub struct DelayFuture<F>(F,bool);

impl<F: Future> Future for DelayFuture<F> {
    type Item = F::Item;
    type Error = F::Error;

    fn poll(&mut self, cx: &mut task::Context) -> Poll<F::Item,F::Error> {
        if self.1 {
            self.0.poll(cx)
        } else {
            self.1 = true;
            cx.waker().wake();
            Ok(Async::Pending)
        }
    }
}

/// Introduces one `Ok(Async::Pending)` before polling the given future
pub fn delay_future<F>(f: F) -> DelayFuture<F::Future>
    where F: IntoFuture,
{
    DelayFuture(f.into_future(), false)
}

