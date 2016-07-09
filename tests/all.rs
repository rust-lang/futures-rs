extern crate futures;
extern crate support;

use std::sync::{Mutex, Arc};
use std::sync::mpsc::{channel, TryRecvError};

use futures::*;
use support::*;

fn unselect<T, U, E>(r: Result<(T, U), (E, U)>) -> Result<T, E> {
    match r {
        Ok((t, _)) => Ok(t),
        Err((e, _)) => Err(e),
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
    assert_done(|| f_ok(1).select(f_err(2)).then(unselect), ok(1));
    assert_done(|| f_ok(1).select(Ok(2)).then(unselect), ok(1));
    assert_done(|| f_err(1).select(f_ok(1)).then(unselect), err(1));
    assert_done(|| f_ok(1).select(empty()).then(unselect), Ok(1));
    assert_done(|| empty().select(f_ok(1)).then(unselect), Ok(1));
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

#[test]
fn smoke_promise() {
    assert_done(|| {
        let (c, p) = promise();
        c.finish(1);
        p
    }, ok(1));
    assert_done(|| {
        let (c, p) = promise();
        c.fail(1);
        p
    }, err(1));
    let mut completes = Vec::new();
    assert_empty(|| {
        let (a, b) = promise::<i32, u32>();
        completes.push(a);
        b
    });

    let (c, mut p) = promise::<i32, u32>();
    drop(c);
    assert_panic(p.poll(&Tokens::all()).expect("should be done"));
    assert_panic(p.poll(&Tokens::all()).expect("should be done2"));
    let (tx, rx) = channel();
    let tx = Mutex::new(tx);
    p.schedule(Arc::new(move |_: &Tokens| tx.lock().unwrap().send(()).unwrap()));
    rx.recv().unwrap();
    assert_panic(p.poll(&Tokens::all()).expect("should be done2"));
}

#[test]
fn select_cancels() {
    let ((a, b), (c, d)) = (promise::<i32, u32>(), promise::<i32, u32>());
    let ((btx, brx), (dtx, drx)) = (channel(), channel());
    let b = b.map(move |b| { btx.send(b).unwrap(); b });
    let d = d.map(move |d| { dtx.send(d).unwrap(); d });

    let mut f = b.select(d).then(unselect);
    // assert!(f.poll(&Tokens::all()).is_none());
    assert!(brx.try_recv().is_err());
    assert!(drx.try_recv().is_err());
    a.finish(1);
    // f.schedule(|_| ());
    assert!(f.poll(&Tokens::all()).is_some());
    assert_eq!(brx.recv().unwrap(), 1);
    drop((c, f));
    assert!(drx.recv().is_err());

    let ((a, b), (c, d)) = (promise::<i32, u32>(), promise::<i32, u32>());
    let ((btx, _brx), (dtx, drx)) = (channel(), channel());
    let b = b.map(move |b| { btx.send(b).unwrap(); b });
    let d = d.map(move |d| { dtx.send(d).unwrap(); d });

    let (tx, rx) = channel();
    let tx = Mutex::new(tx);
    let mut f = b.select(d).then(unselect);
    assert!(f.poll(&Tokens::all()).is_none());
    f.schedule(Arc::new(move |_: &Tokens| tx.lock().unwrap().send(()).unwrap()));
    assert!(rx.try_recv().is_err());
    a.finish(1);
    assert!(rx.recv().is_ok());
    assert!(f.poll(&Tokens::all()).is_some());
    assert_panic(f.poll(&Tokens::all()).unwrap());
    drop((c, f));
    assert!(drx.recv().is_err());
}

#[test]
fn join_cancels() {
    let ((a, b), (c, d)) = (promise::<i32, u32>(), promise::<i32, u32>());
    let ((btx, _brx), (dtx, drx)) = (channel(), channel());
    let b = b.map(move |b| { btx.send(b).unwrap(); b });
    let d = d.map(move |d| { dtx.send(d).unwrap(); d });

    let mut f = b.join(d);
    a.fail(1);
    assert!(f.poll(&Tokens::all()).is_some());
    drop((c, f));
    assert!(drx.recv().is_err());

    let ((a, b), (c, d)) = (promise::<i32, u32>(), promise::<i32, u32>());
    let ((btx, _brx), (dtx, drx)) = (channel(), channel());
    let b = b.map(move |b| { btx.send(b).unwrap(); b });
    let d = d.map(move |d| { dtx.send(d).unwrap(); d });

    let (tx, rx) = channel();
    let tx = Mutex::new(tx);
    let mut f = b.join(d);
    f.schedule(Arc::new(move |_: &Tokens| tx.lock().unwrap().send(()).unwrap()));
    assert!(rx.try_recv().is_err());
    a.fail(1);
    assert!(f.poll(&Tokens::all()).is_some());
    assert_panic(f.poll(&Tokens::all()).unwrap());
    drop((c, f));
    assert!(drx.recv().is_err());
}

#[test]
fn join_incomplete() {
    let (a, b) = promise::<i32, u32>();
    let mut f = f_ok(1).join(b);
    assert!(f.poll(&Tokens::all()).is_none());
    let (tx, rx) = channel();
    f.map(move |r| tx.send(r).unwrap()).forget();
    assert!(rx.try_recv().is_err());
    a.finish(2);
    assert_eq!(rx.recv().unwrap(), (1, 2));

    let (a, b) = promise::<i32, u32>();
    let mut f = b.join(f_ok(2));
    assert!(f.poll(&Tokens::all()).is_none());
    let (tx, rx) = channel();
    f.map(move |r| tx.send(r).unwrap()).forget();
    assert!(rx.try_recv().is_err());
    a.finish(1);
    assert_eq!(rx.recv().unwrap(), (1, 2));

    let (a, b) = promise::<i32, u32>();
    let mut f = f_ok(1).join(b);
    assert!(f.poll(&Tokens::all()).is_none());
    let (tx, rx) = channel();
    f.map_err(move |r| tx.send(r).unwrap()).forget();
    assert!(rx.try_recv().is_err());
    a.fail(2);
    assert_eq!(rx.recv().unwrap(), 2);

    let (a, b) = promise::<i32, u32>();
    let mut f = b.join(f_ok(2));
    assert!(f.poll(&Tokens::all()).is_none());
    let (tx, rx) = channel();
    f.map_err(move |r| tx.send(r).unwrap()).forget();
    assert!(rx.try_recv().is_err());
    a.fail(1);
    assert_eq!(rx.recv().unwrap(), 1);
}

#[test]
fn cancel_propagates() {
    let mut f = promise::<i32, u32>().1.then(|_| -> Done<i32, u32> { panic!() });
    assert_panic(f.poll(&Tokens::all()).unwrap());
    let mut f = promise::<i32, u32>().1.and_then(|_| -> Done<i32, u32> { panic!() });
    assert_panic(f.poll(&Tokens::all()).unwrap());
    let mut f = promise::<i32, u32>().1.or_else(|_| -> Done<i32, u32> { panic!() });
    assert_panic(f.poll(&Tokens::all()).unwrap());
    let mut f = promise::<i32, u32>().1.map(|_| panic!());
    assert_panic(f.poll(&Tokens::all()).unwrap());
    let mut f = promise::<i32, u32>().1.map_err(|_| panic!());
    assert_panic(f.poll(&Tokens::all()).unwrap());
}

#[test]
fn collect_collects() {
    assert_done(|| collect(vec![f_ok(1), f_ok(2)]), Ok(vec![1, 2]));
    assert_done(|| collect(vec![f_ok(1)]), Ok(vec![1]));
    assert_done(|| collect(Vec::<Result<i32, u32>>::new()), Ok(vec![]));

    // TODO: needs more tests
}

#[test]
fn select2() {
    fn d<T, U, E>(r: Result<(T, U), (E, U)>) -> Result<T, E> {
        match r {
            Ok((t, _u)) => Ok(t),
            Err((e, _u)) => Err(e),
        }
    }

    assert_done(|| f_ok(2).select(empty()).then(d), Ok(2));
    assert_done(|| empty().select(f_ok(2)).then(d), Ok(2));
    assert_done(|| f_err(2).select(empty()).then(d), Err(2));
    assert_done(|| empty().select(f_err(2)).then(d), Err(2));

    assert_done(|| {
        f_ok(1).select(f_ok(2))
               .map_err(|_| 0)
               .and_then(|(a, b)| b.map(move |b| a + b))
    }, Ok(3));

    // Finish one half of a select and then fail the second, ensuring that we
    // get the notification of the second one.
    {
        let ((a, b), (c, d)) = (promise::<i32, u32>(), promise::<i32, u32>());
        let f = b.select(d);
        let (tx, rx) = channel();
        f.map(move |r| tx.send(r).unwrap()).forget();
        a.finish(1);
        let (val, next) = rx.recv().unwrap();
        assert_eq!(val, 1);
        let (tx, rx) = channel();
        next.map_err(move |r| tx.send(r).unwrap()).forget();
        assert_eq!(rx.try_recv().err().unwrap(), TryRecvError::Empty);
        c.fail(2);
        assert_eq!(rx.recv().unwrap(), 2);
    }

    // Fail the second half and ensure that we see the first one finish
    {
        let ((a, b), (c, d)) = (promise::<i32, u32>(), promise::<i32, u32>());
        let f = b.select(d);
        let (tx, rx) = channel();
        f.map_err(move |r| tx.send(r).unwrap()).forget();
        c.fail(1);
        let (val, next) = rx.recv().unwrap();
        assert_eq!(val, 1);
        let (tx, rx) = channel();
        next.map(move |r| tx.send(r).unwrap()).forget();
        assert_eq!(rx.try_recv().err().unwrap(), TryRecvError::Empty);
        a.finish(2);
        assert_eq!(rx.recv().unwrap(), 2);
    }

    // Cancelling the first half should cancel the second
    {
        let ((_a, b), (_c, d)) = (promise::<i32, u32>(), promise::<i32, u32>());
        let ((btx, brx), (dtx, drx)) = (channel(), channel());
        let b = b.map(move |v| { btx.send(v).unwrap(); v });
        let d = d.map(move |v| { dtx.send(v).unwrap(); v });
        let f = b.select(d);
        drop(f);
        assert!(drx.recv().is_err());
        assert!(brx.recv().is_err());
    }

    // Cancel after a schedule
    {
        let ((_a, b), (_c, d)) = (promise::<i32, u32>(), promise::<i32, u32>());
        let ((btx, brx), (dtx, drx)) = (channel(), channel());
        let b = b.map(move |v| { btx.send(v).unwrap(); v });
        let d = d.map(move |v| { dtx.send(v).unwrap(); v });
        let mut f = b.select(d);
        f.schedule(Arc::new(|_: &Tokens| ()));
        drop(f);
        assert!(drx.recv().is_err());
        assert!(brx.recv().is_err());
    }

    // Cancel propagates
    {
        let ((a, b), (_c, d)) = (promise::<i32, u32>(), promise::<i32, u32>());
        let ((btx, brx), (dtx, drx)) = (channel(), channel());
        let b = b.map(move |v| { btx.send(v).unwrap(); v });
        let d = d.map(move |v| { dtx.send(v).unwrap(); v });
        let (tx, rx) = channel();
        b.select(d).map(move |_| tx.send(()).unwrap()).forget();
        drop(a);
        assert!(drx.recv().is_err());
        assert!(brx.recv().is_err());
        assert!(rx.recv().is_err());
    }

    // Cancel on early drop
    {
        let (tx, rx) = channel();
        let f = f_ok(1).select(empty().map(move |()| {
            tx.send(()).unwrap();
            1
        }));
        drop(f);
        assert!(rx.recv().is_err());
    }
}
