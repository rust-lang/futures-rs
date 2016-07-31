#![allow(dead_code)]

use std::fmt;
use futures::*;
use futures::stream::Stream;

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
    let mut a = f();
    assert_eq!(&a.poll(&mut Task::new()).unwrap(), &result);
    drop(a);
}

pub fn assert_empty<T: Future, F: FnMut() -> T>(mut f: F) {
    assert!(f().poll(&mut Task::new()).is_not_ready());

    let mut a = f();
    let mut task = Task::new();
    a.schedule(&mut task);
    assert!(a.poll(&mut task).is_not_ready());
    drop(a);
}

pub fn sassert_done<S: Stream>(s: &mut S) {
    match s.poll(&mut Task::new()) {
        Poll::Ok(None) => {}
        Poll::Ok(Some(_)) => panic!("stream had more elements"),
        Poll::Err(_) => panic!("stream had an error"),
        Poll::NotReady => panic!("stream wasn't ready"),
    }
}

pub fn sassert_empty<S: Stream>(s: &mut S) {
    match s.poll(&mut Task::new()) {
        Poll::Ok(None) => panic!("stream is at its end"),
        Poll::Ok(Some(_)) => panic!("stream had more elements"),
        Poll::Err(_) => panic!("stream had an error"),
        Poll::NotReady => {}
    }
}

pub fn sassert_next<S: Stream>(s: &mut S, item: S::Item)
    where S::Item: Eq + fmt::Debug
{
    match s.poll(&mut Task::new()) {
        Poll::Ok(None) => panic!("stream is at its end"),
        Poll::Ok(Some(e)) => assert_eq!(e, item),
        Poll::Err(_) => panic!("stream had an error"),
        Poll::NotReady => panic!("stream wasn't ready"),
    }
}
