#![allow(dead_code)]

use std::fmt;
use std::sync::Arc;
use std::thread;

use futures::{Future, Done, IntoFuture, Async};
use futures::stream::Stream;
use futures::task::{self, Unpark};

pub fn f_ok(a: i32) -> Done<i32, u32> { Ok(a).into_future() }
pub fn f_err(a: u32) -> Done<i32, u32> { Err(a).into_future() }
pub fn ok(a: i32) -> Result<i32, u32> { Ok(a) }
pub fn err(a: u32) -> Result<i32, u32> { Err(a) }

pub fn assert_done<T, F>(mut f: F, result: Result<T::Item, T::Error>)
    where T: Future,
          T::Item: Eq + fmt::Debug,
          T::Error: Eq + fmt::Debug,
          F: FnMut() -> T,
{
    assert_eq!(f().wait(), result);
}

pub fn assert_empty<T: Future, F: FnMut() -> T>(mut f: F) {
    assert!(task::spawn(f()).poll_future(unpark_panic()).ok().unwrap().is_not_ready());
}

pub fn sassert_done<S: Stream>(s: &mut S) {
    match task::spawn(s).poll_stream(unpark_panic()) {
        Ok(Async::Ready(None)) => {}
        Ok(Async::Ready(Some(_))) => panic!("stream had more elements"),
        Ok(Async::NotReady) => panic!("stream wasn't ready"),
        Err(_) => panic!("stream had an error"),
    }
}

pub fn sassert_empty<S: Stream>(s: &mut S) {
    match task::spawn(s).poll_stream(unpark_noop()) {
        Ok(Async::Ready(None)) => panic!("stream is at its end"),
        Ok(Async::Ready(Some(_))) => panic!("stream had more elements"),
        Ok(Async::NotReady) => {}
        Err(_) => panic!("stream had an error"),
    }
}

pub fn sassert_next<S: Stream>(s: &mut S, item: S::Item)
    where S::Item: Eq + fmt::Debug
{
    match task::spawn(s).poll_stream(unpark_panic()) {
        Ok(Async::Ready(None)) => panic!("stream is at its end"),
        Ok(Async::Ready(Some(e))) => assert_eq!(e, item),
        Ok(Async::NotReady) => panic!("stream wasn't ready"),
        Err(_) => panic!("stream had an error"),
    }
}

pub fn unpark_panic() -> Arc<Unpark> {
    struct Foo;

    impl Unpark for Foo {
        fn unpark(&self) {
            panic!("should not be unparked");
        }
    }

    Arc::new(Foo)
}

pub fn unpark_noop() -> Arc<Unpark> {
    struct Foo;

    impl Unpark for Foo {
        fn unpark(&self) {}
    }

    Arc::new(Foo)
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
        thread::spawn(|| self.wait());
    }
}
