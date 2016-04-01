extern crate futures;

use std::sync::mpsc::channel;
use std::fmt;

use futures::*;

fn is_future_v<A, B, C>(_: C)
    where A: Send + 'static,
          B: Send + 'static,
          C: Future<Item=A, Error=B>
{}

fn get<F: Future>(mut f: F) -> Result<F::Item, F::Error> {
    unwrap(f.poll().expect("future not ready"))
}

fn unwrap<A, B>(r: PollResult<A, B>) -> Result<A, B> {
    match r {
        Ok(e) => Ok(e),
        Err(PollError::Canceled) => panic!("future canceled"),
        Err(PollError::Panicked(p)) => panic!(p),
        Err(PollError::Other(e)) => Err(e),
    }
}


fn f_ok(a: i32) -> Done<i32, u32> { Ok(a).into_future() }
fn f_err(a: u32) -> Done<i32, u32> { Err(a).into_future() }
fn ok(a: i32) -> Result<i32, u32> { Ok(a) }
fn err(a: u32) -> Result<i32, u32> { Err(a) }

fn assert_done<T: Future, F: Fn() -> T>(f: F, result: Result<T::Item, T::Error>)
    where T::Item: Eq + fmt::Debug, T::Error: Eq + fmt::Debug
{
    assert_eq!(&get(f()), &result);
    let (tx, rx) = channel();
    f().schedule(move |r| tx.send(r).unwrap());
    assert_eq!(&unwrap(rx.recv().unwrap()), &result);
}

fn assert_empty<T: Future, F: Fn() -> T>(f: F) {
    let mut a = f();
    assert!(a.poll().is_none());
    a.cancel();
    assert_cancel(a.poll().expect("cancel should force a finish"));

    let mut a = f();
    a.cancel();
    assert_cancel(a.poll().expect("cancel should force a finish2"));

    let (tx, rx) = channel();
    f().schedule(move |r| tx.send(r).unwrap());
    assert!(rx.try_recv().is_err());

    let (tx, rx) = channel();
    let mut a = f();
    a.schedule(move |r| tx.send(r).unwrap());
    a.cancel();
    assert_cancel(rx.recv().unwrap());

    let (tx, rx) = channel();
    let mut a = f();
    a.cancel();
    a.schedule(move |r| tx.send(r).unwrap());
    assert_cancel(rx.recv().unwrap());
}

fn assert_cancel<T, E>(r: PollResult<T, E>) {
    match r {
        Ok(_) => panic!("can't succeed after cancel"),
        Err(PollError::Canceled) => {}
        Err(PollError::Panicked(_)) => panic!("panic error, not cancel"),
        Err(PollError::Other(_)) => panic!("normal error, not cancel"),
    }
}

#[test]
fn result_smoke() {
    is_future_v::<i32, u32, _>(f_ok(1).map(|a| a + 1));
    is_future_v::<i32, u32, _>(f_ok(1).map_err(|a| a + 1));
    is_future_v::<i32, u32, _>(f_ok(1).and_then(|a| Ok(a)));
    is_future_v::<i32, u32, _>(f_ok(1).or_else(|a| Err(a)));
    is_future_v::<i32, u32, _>(f_ok(1).select(Err(3)));
    is_future_v::<(i32, i32), u32, _>(f_ok(1).join(Err(3)));
    is_future_v::<i32, u32, _>(f_ok(1).map(move |a| f_ok(a)).flatten());

    assert_done(|| f_ok(1).map(|a| a + 2), ok(3));
    assert_done(|| f_err(1).map(|a| a + 2), err(1));
    assert_done(|| f_ok(1).map_err(|a| a + 2), ok(1));
    assert_done(|| f_err(1).map_err(|a| a + 2), err(3));
    assert_done(|| f_ok(1).and_then(|a| Ok(a + 2)), ok(3));
    assert_done(|| f_err(1).and_then(|a| Ok(a + 2)), err(1));
    assert_done(|| f_ok(1).and_then(|a| Err(a as u32 + 3)), err(4));
    assert_done(|| f_err(1).and_then(|a| Err(a as u32 + 4)), err(1));
    assert_done(|| f_ok(1).or_else(|a| Ok(a as i32 + 2)), ok(1));
    assert_done(|| f_err(1).or_else(|a| Ok(a as i32 + 2)), ok(3));
    assert_done(|| f_ok(1).or_else(|a| Err(a + 3)), ok(1));
    assert_done(|| f_err(1).or_else(|a| Err(a + 4)), err(5));
    assert_done(|| f_ok(1).select(f_err(2)), ok(1));
    assert_done(|| f_ok(1).select(Ok(2)), ok(1));
    assert_done(|| f_err(1).select(f_ok(1)), err(1));
    assert_done(|| f_ok(1).select(empty()), Ok(1));
    assert_done(|| empty().select(f_ok(1)), Ok(1));
    assert_done(|| f_ok(1).join(f_err(1)), Err(1));
    assert_done(|| f_ok(1).join(Ok(2)), Ok((1, 2)));
    assert_done(|| f_err(1).join(f_ok(1)), Err(1));
    assert_done(|| f_ok(1).then(|_| Ok(2)), ok(2));
    assert_done(|| f_ok(1).then(|_| Err(2)), err(2));
    assert_done(|| f_err(1).then(|_| Ok(2)), ok(2));
    assert_done(|| f_err(1).then(|_| Err(2)), err(2));
}

#[test]
fn test_empty() {
    fn empty() -> Empty<i32, u32> { futures::empty() }

    assert_empty(|| empty().select(empty()));
    assert_empty(|| empty().join(empty()));
    assert_empty(|| empty().join(f_ok(1)));
    assert_empty(|| f_ok(1).join(empty()));
    assert_empty(|| empty().or_else(move |_| empty()));
    assert_empty(|| empty().and_then(move |_| empty()));
    assert_empty(|| f_err(1).or_else(move |_| empty()));
    assert_empty(|| f_ok(1).and_then(move |_| empty()));
    assert_empty(|| empty().map(|a| a + 1));
    assert_empty(|| empty().map_err(|a| a + 1));
    assert_empty(|| empty().then(|a| a));
}

// #[test]
// fn test_collect() {
//     let f_ok1: FutureResult<i32, i32> = Ok(1).into_future();
//     let f_ok2: FutureResult<i32, i32> = Ok(2).into_future();
//     let f_ok3: FutureResult<i32, i32> = Ok(3).into_future();
//     let f_err1: FutureResult<i32, i32> = Err(1).into_future();
//
//     assert_eq!(get(collect(vec![f_ok1, f_ok2, f_ok3])), Ok(vec![1, 2, 3]));
//     assert_eq!(get(collect(vec![f_ok1, f_err1, f_ok3])), Err(1));
// }
//
// #[test]
// fn test_finished() {
//     assert_eq!(get(finished::<_, i32>(1)), Ok(1));
//     assert_eq!(get(failed::<i32, _>(1)), Err(1));
// }
//
// #[test]
// fn flatten() {
//     assert_eq!(get(finished::<_, i32>(finished::<_, i32>(1)).flatten()), Ok(1));
//     assert_eq!(get(finished::<_, i32>(failed::<i32, _>(1)).flatten()), Err(1));
//     assert_eq!(get(failed(1).map(finished::<i32, _>).flatten()), Err(1));
//     assert_eq!(get(finished::<_, i8>(finished::<_, i32>(1)).flatten()), Ok(1));
//     assert!(finished::<_, i8>(empty::<i8, i8>()).flatten().poll().is_err());
//     assert!(empty::<i8, i8>().map(finished::<_, i8>).flatten().poll().is_err());
//
// }
//
// #[test]
// fn await() {
//     let f_ok: FutureResult<i32, i32> = Ok(1).into_future();
//     let f_err: FutureResult<i32, i32> = Err(1).into_future();
//
//     fn ok(a: i32) -> Result<i32, i32> { Ok(a) }
//     fn err(a: i32) -> Result<i32, i32> { Err(a) }
//
//     assert_eq!(f_ok.map(|a| a + 2).await(), ok(3));
//     assert_eq!(f_err.map(|a| a + 2).await(), err(1));
//     assert_eq!(f_ok.map_err(|a| a + 2).await(), ok(1));
//     assert_eq!(f_err.map_err(|a| a + 2).await(), err(3));
//     assert_eq!(f_ok.and_then(|a| Ok(a + 2)).await(), ok(3));
//     assert_eq!(f_err.and_then(|a| Ok(a + 2)).await(), err(1));
//     assert_eq!(f_ok.and_then(|a| Err(a + 3)).await(), err(4));
//     assert_eq!(f_err.and_then(|a| Err(a + 4)).await(), err(1));
//     assert_eq!(f_ok.or_else(|a| Ok(a + 2)).await(), ok(1));
//     assert_eq!(f_err.or_else(|a| Ok(a + 2)).await(), ok(3));
//     assert_eq!(f_ok.or_else(|a| Err(a + 3)).await(), ok(1));
//     assert_eq!(f_err.or_else(|a| Err(a + 4)).await(), err(5));
//     assert_eq!(f_ok.select(f_err).await(), ok(1));
//     assert_eq!(f_ok.select(Ok(2)).await(), ok(1));
//     assert_eq!(f_err.select(f_ok).await(), err(1));
//     assert_eq!(f_ok.select(empty()).await(), Ok(1));
//     assert_eq!(empty().select(f_ok).await(), Ok(1));
//     assert_eq!(f_ok.join(f_err).await(), Err(1));
//     assert_eq!(f_ok.join(Ok(2)).await(), Ok((1, 2)));
//     assert_eq!(f_err.join(f_ok).await(), Err(1));
//     assert_eq!(f_ok.then(|_| Ok(2)).await(), ok(2));
//     assert_eq!(f_ok.then(|_| Err(2)).await(), err(2));
//     assert_eq!(f_err.then(|_| Ok(2)).await(), ok(2));
//     assert_eq!(f_err.then(|_| Err(2)).await(), err(2));
// }
//
// fn assert_empty<F: Future>(f: F) -> F {
//     match f.poll() {
//         Ok(..) => panic!("future is full"),
//         Err(f) => f,
//     }
// }
//
// #[test]
// fn needs_progress() {
//     let f_ok: FutureResult<i32, i32> = Ok(1).into_future();
//     let f_err: FutureResult<i32, i32> = Err(1).into_future();
//
//     let (p, c) = promise::pair::<i32, i32>();
//     let f = assert_empty(f_ok.then(|_| p));
//     let f = assert_empty(f);
//     c.finish(1);
//     assert_eq!(f.await(), Ok(1));
//
//     let (p, c) = promise::pair();
//     let (tx, rx) = channel();
//     f_ok.and_then(|_| p).schedule(move |r| tx.send(r).unwrap());
//     assert!(rx.try_recv().is_err());
//     c.finish(1);
//     assert_eq!(rx.recv().unwrap(), Ok(1));
//
//     let (p, c) = promise::pair();
//     let f = assert_empty(f_ok.and_then(|_| p));
//     let f = assert_empty(f);
//     c.finish(1);
//     assert_eq!(f.await(), Ok(1));
//
//     let (p, c) = promise::pair::<i32, i32>();
//     let f = assert_empty(f_err.or_else(|_| p));
//     let f = assert_empty(f);
//     c.finish(1);
//     assert_eq!(f.await(), Ok(1));
//
//     let (p, c) = promise::pair::<i32, i32>();
//     let (tx, rx) = channel();
//     f_err.or_else(|_| p).schedule(move |r| tx.send(r).unwrap());
//     assert!(rx.try_recv().is_err());
//     c.finish(1);
//     assert_eq!(rx.recv().unwrap(), Ok(1));
//
//     let (p, c) = promise::pair::<i32, i32>();
//     let f = assert_empty(f_ok.map(|_| p).flatten());
//     let f = assert_empty(f);
//     c.finish(1);
//     assert_eq!(f.await(), Ok(1));
//
//     let (p, c) = promise::pair::<i32, i32>();
//     let (tx, rx) = channel();
//     f_ok.map(|_| p).flatten().schedule(move |r| tx.send(r).unwrap());
//     assert!(rx.try_recv().is_err());
//     c.finish(1);
//     assert_eq!(rx.recv().unwrap(), Ok(1));
//
//     let (p, _c) = promise::pair::<i32, i32>();
//     let (tx, rx) = channel();
//     f_err.map(|_| p).flatten().schedule(move |r| tx.send(r).unwrap());
//     assert_eq!(rx.recv().unwrap(), Err(1));
//
//     let (p, c) = promise::pair::<i32, i32>();
//     let (tx, rx) = channel();
//     f_ok.map(|_| p).flatten().schedule(move |r| tx.send(r).unwrap());
//     assert!(rx.try_recv().is_err());
//     c.fail(2);
//     assert_eq!(rx.recv().unwrap(), Err(2));
//
//     let (p1, c1) = promise::pair::<i32, i32>();
//     let (p2, c2) = promise::pair::<i32, i32>();
//     let f = assert_empty(p1.join(p2));
//     c1.finish(1);
//     let f = assert_empty(f);
//     let f = assert_empty(f);
//     c2.finish(2);
//     assert_eq!(f.await(), Ok((1, 2)));
//
//     let (p1, c1) = promise::pair::<i32, i32>();
//     let (p2, c2) = promise::pair::<i32, i32>();
//     let f = assert_empty(p1.join(p2));
//     c2.finish(2);
//     let f = assert_empty(f);
//     let f = assert_empty(f);
//     c1.finish(1);
//     assert_eq!(f.await(), Ok((1, 2)));
//
//     let (p1, c1) = promise::pair::<i32, i32>();
//     let (p2, c2) = promise::pair::<i32, i32>();
//     let f = assert_empty(p1.join(p2));
//     c2.finish(2);
//     let f = assert_empty(f);
//     let f = assert_empty(f);
//     c1.finish(1);
//     assert_eq!(get(f), Ok((1, 2)));
//
//     let (p1, c1) = promise::pair::<i32, i32>();
//     let (p2, c2) = promise::pair::<i32, i32>();
//     let f = assert_empty(p1.join(p2));
//     c1.finish(1);
//     let f = assert_empty(f);
//     let f = assert_empty(f);
//     c2.finish(2);
//     assert_eq!(get(f), Ok((1, 2)));
//
//     let (p1, c1) = promise::pair::<i32, i32>();
//     let (p2, c2) = promise::pair::<i32, i32>();
//     let f = assert_empty(p1.join(p2));
//     c1.finish(1);
//     let f = assert_empty(f);
//     let f = assert_empty(f);
//     c2.fail(2);
//     assert_eq!(f.await(), Err(2));
// }
//
// #[test]
// fn collect_progress() {
//     let (p1, c1) = promise::pair::<i32, i32>();
//     let (p2, c2) = promise::pair::<i32, i32>();
//     let f = collect(vec![p1, p2]);
//     let f = assert_empty(f);
//     c1.finish(1);
//     let f = assert_empty(assert_empty(f));
//     c2.finish(2);
//     assert_eq!(f.await(), Ok(vec![1, 2]));
//
//     let (p1, c1) = promise::pair::<i32, i32>();
//     let (p2, c2) = promise::pair::<i32, i32>();
//     let (tx, rx) = channel();
//     collect(vec![p1, p2]).schedule(move |r| tx.send(r).unwrap());
//     assert!(rx.try_recv().is_err());
//     c1.finish(1);
//     assert!(rx.try_recv().is_err());
//     c2.finish(2);
//     assert_eq!(rx.recv().unwrap(), Ok(vec![1, 2]));
//
//     let (p1, c1) = promise::pair::<i32, i32>();
//     let (p2, c2) = promise::pair::<i32, i32>();
//     let (tx, rx) = channel();
//     collect(vec![p1, p2]).schedule(move |r| tx.send(r).unwrap());
//     assert!(rx.try_recv().is_err());
//     c1.finish(1);
//     assert!(rx.try_recv().is_err());
//     c2.fail(2);
//     assert_eq!(rx.recv().unwrap(), Err(2));
//
//     let (p1, c1) = promise::pair::<i32, i32>();
//     let (p2, _c2) = promise::pair::<i32, i32>();
//     let f = collect(vec![p1, p2]);
//     c1.fail(1);
//     assert_eq!(get(f), Err(1));
// }
