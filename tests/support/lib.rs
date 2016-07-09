extern crate futures;

use std::fmt;
use std::sync::Arc;

use futures::*;
use futures::stream::Stream;

pub fn unwrap<A, B>(r: PollResult<A, B>) -> Result<A, B> {
    match r {
        Ok(e) => Ok(e),
        Err(PollError::Panicked(p)) => panic!(p),
        Err(PollError::Other(e)) => Err(e),
    }
}


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
    assert_eq!(&unwrap(a.poll(&Tokens::all()).unwrap()), &result);
    drop(a);

    let mut a = f();
    assert!(a.poll(&Tokens::all()).is_some());
    assert_panic(a.poll(&Tokens::all()).unwrap());
    drop(a);
}

pub fn assert_empty<T: Future, F: FnMut() -> T>(mut f: F) {
    assert!(f().poll(&Tokens::all()).is_none());

    let mut a = f();
    a.schedule(Arc::new(move |_: &Tokens| ()));
    assert!(a.poll(&Tokens::all()).is_none());
    drop(a);
}

pub fn assert_panic<T, E>(r: PollResult<T, E>) {
    match r {
        Ok(_) => panic!("can't succeed after panic"),
        Err(PollError::Panicked(_)) => {}
        Err(PollError::Other(_)) => panic!("other error, not panic"),
    }
}

pub fn sassert_done<S: Stream>(s: &mut S) {
    match s.poll(&Tokens::all()) {
        Some(Ok(None)) => {}
        Some(Ok(Some(_))) => panic!("stream had more elements"),
        Some(Err(PollError::Other(_))) => panic!("stream had an error"),
        Some(Err(PollError::Panicked(_))) => panic!("stream panicked"),
        None => panic!("stream wasn't ready"),
    }
}

pub fn sassert_empty<S: Stream>(s: &mut S) {
    match s.poll(&Tokens::all()) {
        Some(Ok(None)) => panic!("stream is at its end"),
        Some(Ok(Some(_))) => panic!("stream had more elements"),
        Some(Err(PollError::Other(_))) => panic!("stream had an error"),
        Some(Err(PollError::Panicked(_))) => panic!("stream panicked"),
        None => {}
    }
}

pub fn sassert_next<S: Stream>(s: &mut S, item: S::Item)
    where S::Item: Eq + fmt::Debug
{
    match s.poll(&Tokens::all()) {
        Some(Ok(None)) => panic!("stream is at its end"),
        Some(Ok(Some(e))) => assert_eq!(e, item),
        Some(Err(PollError::Other(_))) => panic!("stream had an error"),
        Some(Err(PollError::Panicked(_))) => panic!("stream panicked"),
        None => panic!("stream wasn't ready"),
    }
}
