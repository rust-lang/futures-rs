extern crate futures;

use std::sync::mpsc::channel;
use std::fmt;

use futures::*;

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

fn assert_done<T, F>(mut f: F, result: Result<T::Item, T::Error>)
    where T: Future,
          T::Item: Eq + fmt::Debug,
          T::Error: Eq + fmt::Debug,
          F: FnMut() -> T,
{
    let mut a = f();
    assert_eq!(&unwrap(a.poll().expect("future not ready")), &result);
    assert_bad(a.poll().expect("future should still be ready"));

    let (tx, rx) = channel();
    f().schedule(move |r| tx.send(r).unwrap());
    assert_eq!(&unwrap(rx.recv().unwrap()), &result);

    let mut a = f();
    a.schedule(|_| ());
    let (tx, rx) = channel();
    a.schedule(move |r| tx.send(r).unwrap());
    assert_panic(rx.recv().unwrap());
}

fn assert_empty<T: Future, F: FnMut() -> T>(mut f: F) {
    let mut a = f();
    assert!(a.poll().is_none());
    a.cancel();
    assert_cancel(a.poll().expect("cancel should force a finish"));

    let mut a = f();
    a.cancel();
    assert_cancel(a.poll().expect("cancel should force a finish2"));

    let (tx, rx) = channel();
    f().schedule(move |r| drop(tx.send(r)));
    assert!(rx.try_recv().is_err());

    let (tx, rx) = channel();
    let (tx2, rx2) = channel();
    let mut a = f();
    a.schedule(move |r| tx.send(r).unwrap());
    a.schedule(move |r| tx2.send(r).unwrap());
    assert_panic(rx2.recv().unwrap());
    a.cancel();
    assert_cancel(rx.recv().unwrap());

    let (tx, rx) = channel();
    let mut a = f();
    a.cancel();
    let tx2 = tx.clone();
    a.schedule(move |r| tx2.send(r).unwrap());
    assert_cancel(rx.recv().unwrap());
    a.schedule(move |r| tx.send(r).unwrap());
    assert_bad(rx.recv().unwrap());
}

fn assert_cancel<T, E>(r: PollResult<T, E>) {
    match r {
        Ok(_) => panic!("can't succeed after cancel"),
        Err(PollError::Canceled) => {}
        Err(PollError::Panicked(_)) => panic!("panic error, not cancel"),
        Err(PollError::Other(_)) => panic!("other error, not cancel"),
    }
}

fn assert_panic<T, E>(r: PollResult<T, E>) {
    match r {
        Ok(_) => panic!("can't succeed after panic"),
        Err(PollError::Canceled) => panic!("cancel error, not panic"),
        Err(PollError::Panicked(_)) => {}
        Err(PollError::Other(_)) => panic!("other error, not panic"),
    }
}

fn assert_bad<T, E>(r: PollResult<T, E>) {
    match r {
        Ok(_) => panic!("expected panic, got success"),
        Err(PollError::Other(_)) => panic!("expected panic, got other"),
        Err(PollError::Panicked(_)) => {}
        Err(PollError::Canceled) => {}
    }
}

#[test]
fn result_smoke() {
    fn is_future_v<A, B, C>(_: C)
        where A: Send + 'static,
              B: Send + 'static,
              C: Future<Item=A, Error=B>
    {}

    is_future_v::<i32, u32, _>(f_ok(1).map(|a| a + 1));
    is_future_v::<i32, u32, _>(f_ok(1).map_err(|a| a + 1));
    is_future_v::<i32, u32, _>(f_ok(1).and_then(|a| Ok(a)));
    is_future_v::<i32, u32, _>(f_ok(1).or_else(|a| Err(a)));
    is_future_v::<i32, u32, _>(f_ok(1).select(Err(3)));
    is_future_v::<(i32, i32), u32, _>(f_ok(1).join(Err(3)));
    is_future_v::<i32, u32, _>(f_ok(1).map(move |a| f_ok(a)).flatten());

    assert_done(|| f_ok(1), ok(1));
    assert_done(|| f_err(1), err(1));
    assert_done(|| done(Ok(1)), ok(1));
    assert_done(|| done(Err(1)), err(1));
    assert_done(|| finished(1), ok(1));
    assert_done(|| failed(1), err(1));
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

    assert_empty(|| empty());
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

#[test]
fn test_finished() {
    assert_done(|| finished(1), ok(1));
    assert_done(|| failed(1), err(1));
}

#[test]
fn flatten() {
    fn finished<T: Send + 'static>(a: T) -> Finished<T, u32> {
        futures::finished(a)
    }
    fn failed<E: Send + 'static>(b: E) -> Failed<i32, E> {
        futures::failed(b)
    }

    assert_done(|| finished(finished(1)).flatten(), ok(1));
    assert_done(|| finished(failed(1)).flatten(), err(1));
    assert_done(|| failed(1u32).map(finished).flatten(), err(1));
    assert_done(|| futures::finished::<_, u8>(futures::finished::<_, u32>(1))
                           .flatten(), ok(1));
    assert_empty(|| finished(empty::<i32, u32>()).flatten());
    assert_empty(|| empty::<i32, u32>().map(finished).flatten());
}

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

#[test]
fn smoke_promise() {
    assert_done(|| {
        let (p, c) = promise();
        c.finish(1);
        p
    }, ok(1));
    assert_done(|| {
        let (p, c) = promise();
        c.fail(1);
        p
    }, err(1));
    let mut completes = Vec::new();
    assert_empty(|| {
        let (a, b) = promise::<i32, u32>();
        completes.push(b);
        a
    });

    let (mut p, c) = promise::<i32, u32>();
    drop(c);
    assert_cancel(p.poll().unwrap());
    assert_panic(p.poll().unwrap());
    let (tx, rx) = channel();
    p.schedule(move |r| tx.send(r).unwrap());
    assert_panic(rx.recv().unwrap());
}

#[test]
fn select_cancels() {
    let ((a, b), (c, d)) = (promise::<i32, u32>(), promise::<i32, u32>());
    let ((atx, arx), (ctx, crx)) = (channel(), channel());
    let a = a.map(move |a| { atx.send(a).unwrap(); a });
    let c = c.map(move |c| { ctx.send(c).unwrap(); c });

    let mut f = a.select(c);
    assert!(f.poll().is_none());
    assert!(arx.try_recv().is_err());
    assert!(crx.try_recv().is_err());
    b.finish(1);
    assert!(f.poll().is_some());
    assert_eq!(arx.recv().unwrap(), 1);
    drop((d, f));
    assert!(crx.recv().is_err());

    let ((a, b), (c, d)) = (promise::<i32, u32>(), promise::<i32, u32>());
    let ((atx, _arx), (ctx, crx)) = (channel(), channel());
    let a = a.map(move |a| { atx.send(a).unwrap(); a });
    let c = c.map(move |c| { ctx.send(c).unwrap(); c });

    let mut f = a.select(c);
    f.schedule(|_| ());
    assert_panic(f.poll().unwrap());
    b.finish(1);
    drop((d, f));
    assert!(crx.recv().is_err());
}

#[test]
fn join_cancels() {
    let ((a, b), (c, d)) = (promise::<i32, u32>(), promise::<i32, u32>());
    let ((atx, _arx), (ctx, crx)) = (channel(), channel());
    let a = a.map(move |a| { atx.send(a).unwrap(); a });
    let c = c.map(move |c| { ctx.send(c).unwrap(); c });

    let mut f = a.join(c);
    b.fail(1);
    assert!(f.poll().is_some());
    drop((d, f));
    assert!(crx.recv().is_err());

    let ((a, b), (c, d)) = (promise::<i32, u32>(), promise::<i32, u32>());
    let ((atx, _arx), (ctx, crx)) = (channel(), channel());
    let a = a.map(move |a| { atx.send(a).unwrap(); a });
    let c = c.map(move |c| { ctx.send(c).unwrap(); c });

    let mut f = a.join(c);
    f.schedule(|_| ());
    assert_panic(f.poll().unwrap());
    b.fail(1);
    drop((d, f));
    assert!(crx.recv().is_err());
}

#[test]
fn join_incomplete() {
    let (a, b) = promise::<i32, u32>();
    let mut f = f_ok(1).join(a);
    assert!(f.poll().is_none());
    let (tx, rx) = channel();
    f.schedule(move |r| tx.send(r).unwrap());
    assert!(rx.try_recv().is_err());
    b.finish(2);
    assert_eq!(unwrap(rx.recv().unwrap()), Ok((1, 2)));

    let (a, b) = promise::<i32, u32>();
    let mut f = a.join(f_ok(2));
    assert!(f.poll().is_none());
    let (tx, rx) = channel();
    f.schedule(move |r| tx.send(r).unwrap());
    assert!(rx.try_recv().is_err());
    b.finish(1);
    assert_eq!(unwrap(rx.recv().unwrap()), Ok((1, 2)));

    let (a, b) = promise::<i32, u32>();
    let mut f = f_ok(1).join(a);
    assert!(f.poll().is_none());
    let (tx, rx) = channel();
    f.schedule(move |r| tx.send(r).unwrap());
    assert!(rx.try_recv().is_err());
    b.fail(2);
    assert_eq!(unwrap(rx.recv().unwrap()), Err(2));

    let (a, b) = promise::<i32, u32>();
    let mut f = a.join(f_ok(2));
    assert!(f.poll().is_none());
    let (tx, rx) = channel();
    f.schedule(move |r| tx.send(r).unwrap());
    assert!(rx.try_recv().is_err());
    b.fail(1);
    assert_eq!(unwrap(rx.recv().unwrap()), Err(1));
}

#[test]
fn cancel_propagates() {
    let mut f = promise::<i32, u32>().0.then(|_| -> Done<i32, u32> { panic!() });
    assert_cancel(f.poll().unwrap());
    let mut f = promise::<i32, u32>().0.and_then(|_| -> Done<i32, u32> { panic!() });
    assert_cancel(f.poll().unwrap());
    let mut f = promise::<i32, u32>().0.or_else(|_| -> Done<i32, u32> { panic!() });
    assert_cancel(f.poll().unwrap());
    let mut f = promise::<i32, u32>().0.map(|_| panic!());
    assert_cancel(f.poll().unwrap());
    let mut f = promise::<i32, u32>().0.map_err(|_| panic!());
    assert_cancel(f.poll().unwrap());
}
