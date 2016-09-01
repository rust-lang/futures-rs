extern crate futures;

use std::sync::mpsc::{channel, TryRecvError};

use futures::*;

mod support;
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
fn smoke_oneshot() {
    assert_done(|| {
        let (c, p) = oneshot();
        c.complete(1);
        p
    }, Ok(1));
    assert_done(|| {
        let (c, p) = oneshot::<i32>();
        drop(c);
        p
    }, Err(Canceled));
    let mut completes = Vec::new();
    assert_empty(|| {
        let (a, b) = oneshot::<i32>();
        completes.push(a);
        b
    });

    let (c, p) = oneshot::<i32>();
    drop(c);
    let res = task::spawn(p).poll_future(unpark_panic());
    assert!(res.is_err());
    let (c, p) = oneshot::<i32>();
    drop(c);
    let (tx, rx) = channel();
    p.then(move |_| {
        tx.send(())
    }).forget();
    rx.recv().unwrap();
}

#[test]
fn select_cancels() {
    let ((a, b), (c, d)) = (oneshot::<i32>(), oneshot::<i32>());
    let ((btx, brx), (dtx, drx)) = (channel(), channel());
    let b = b.map(move |b| { btx.send(b).unwrap(); b });
    let d = d.map(move |d| { dtx.send(d).unwrap(); d });

    let f = b.select(d).then(unselect);
    // assert!(f.poll(&mut Task::new()).is_not_ready());
    assert!(brx.try_recv().is_err());
    assert!(drx.try_recv().is_err());
    a.complete(1);
    let res = task::spawn(f).poll_future(unpark_panic());
    assert!(res.ok().unwrap().is_ready());
    assert_eq!(brx.recv().unwrap(), 1);
    drop(c);
    assert!(drx.recv().is_err());

    let ((a, b), (c, d)) = (oneshot::<i32>(), oneshot::<i32>());
    let ((btx, _brx), (dtx, drx)) = (channel(), channel());
    let b = b.map(move |b| { btx.send(b).unwrap(); b });
    let d = d.map(move |d| { dtx.send(d).unwrap(); d });

    let mut f = task::spawn(b.select(d).then(unselect));
    assert!(f.poll_future(unpark_noop()).ok().unwrap().is_not_ready());
    assert!(f.poll_future(unpark_noop()).ok().unwrap().is_not_ready());
    a.complete(1);
    assert!(f.poll_future(unpark_panic()).ok().unwrap().is_ready());
    drop((c, f));
    assert!(drx.recv().is_err());
}

#[test]
fn join_cancels() {
    let ((a, b), (c, d)) = (oneshot::<i32>(), oneshot::<i32>());
    let ((btx, _brx), (dtx, drx)) = (channel(), channel());
    let b = b.map(move |b| { btx.send(b).unwrap(); b });
    let d = d.map(move |d| { dtx.send(d).unwrap(); d });

    let f = b.join(d);
    drop(a);
    let res = task::spawn(f).poll_future(unpark_panic());
    assert!(res.is_err());
    drop(c);
    assert!(drx.recv().is_err());

    let ((a, b), (c, d)) = (oneshot::<i32>(), oneshot::<i32>());
    let ((btx, _brx), (dtx, drx)) = (channel(), channel());
    let b = b.map(move |b| { btx.send(b).unwrap(); b });
    let d = d.map(move |d| { dtx.send(d).unwrap(); d });

    let (tx, rx) = channel();
    let f = b.join(d);
    f.then(move |_| {
        tx.send(()).unwrap();
        let res: Result<(), ()> = Ok(());
        res
    }).forget();
    assert!(rx.try_recv().is_err());
    drop(a);
    rx.recv().unwrap();
    drop(c);
    assert!(drx.recv().is_err());
}

#[test]
fn join_incomplete() {
    let (a, b) = oneshot::<i32>();
    let (tx, rx) = channel();
    let mut f = task::spawn(finished(1).join(b).map(move |r| tx.send(r).unwrap()));
    assert!(f.poll_future(unpark_noop()).ok().unwrap().is_not_ready());
    assert!(rx.try_recv().is_err());
    a.complete(2);
    assert!(f.poll_future(unpark_noop()).ok().unwrap().is_ready());
    assert_eq!(rx.recv().unwrap(), (1, 2));

    let (a, b) = oneshot::<i32>();
    let (tx, rx) = channel();
    let mut f = task::spawn(b.join(Ok(2)).map(move |r| tx.send(r).unwrap()));
    assert!(f.poll_future(unpark_noop()).ok().unwrap().is_not_ready());
    assert!(rx.try_recv().is_err());
    a.complete(1);
    assert!(f.poll_future(unpark_noop()).ok().unwrap().is_ready());
    assert_eq!(rx.recv().unwrap(), (1, 2));

    let (a, b) = oneshot::<i32>();
    let (tx, rx) = channel();
    let mut f = task::spawn(finished(1).join(b).map_err(move |_r| tx.send(2).unwrap()));
    assert!(f.poll_future(unpark_noop()).ok().unwrap().is_not_ready());
    assert!(rx.try_recv().is_err());
    drop(a);
    assert!(f.poll_future(unpark_noop()).is_err());
    assert_eq!(rx.recv().unwrap(), 2);

    let (a, b) = oneshot::<i32>();
    let (tx, rx) = channel();
    let mut f = task::spawn(b.join(Ok(2)).map_err(move |_r| tx.send(1).unwrap()));
    assert!(f.poll_future(unpark_noop()).ok().unwrap().is_not_ready());
    assert!(rx.try_recv().is_err());
    drop(a);
    assert!(f.poll_future(unpark_noop()).is_err());
    assert_eq!(rx.recv().unwrap(), 1);
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
        let ((a, b), (c, d)) = (oneshot::<i32>(), oneshot::<i32>());
        let f = b.select(d);
        let (tx, rx) = channel();
        f.map(move |r| tx.send(r).unwrap()).forget();
        a.complete(1);
        let (val, next) = rx.recv().unwrap();
        assert_eq!(val, 1);
        let (tx, rx) = channel();
        next.map_err(move |_r| tx.send(2).unwrap()).forget();
        assert_eq!(rx.try_recv().err().unwrap(), TryRecvError::Empty);
        drop(c);
        assert_eq!(rx.recv().unwrap(), 2);
    }

    // Fail the second half and ensure that we see the first one finish
    {
        let ((a, b), (c, d)) = (oneshot::<i32>(), oneshot::<i32>());
        let f = b.select(d);
        let (tx, rx) = channel();
        f.map_err(move |r| tx.send((1, r.1)).unwrap()).forget();
        drop(c);
        let (val, next) = rx.recv().unwrap();
        assert_eq!(val, 1);
        let (tx, rx) = channel();
        next.map(move |r| tx.send(r).unwrap()).forget();
        assert_eq!(rx.try_recv().err().unwrap(), TryRecvError::Empty);
        a.complete(2);
        assert_eq!(rx.recv().unwrap(), 2);
    }

    // Cancelling the first half should cancel the second
    {
        let ((_a, b), (_c, d)) = (oneshot::<i32>(), oneshot::<i32>());
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
        let ((_a, b), (_c, d)) = (oneshot::<i32>(), oneshot::<i32>());
        let ((btx, brx), (dtx, drx)) = (channel(), channel());
        let b = b.map(move |v| { btx.send(v).unwrap(); v });
        let d = d.map(move |v| { dtx.send(v).unwrap(); v });
        let f = b.select(d);
        drop(task::spawn(f).poll_future(support::unpark_noop()));
        assert!(drx.recv().is_err());
        assert!(brx.recv().is_err());
    }

    // Cancel propagates
    {
        let ((a, b), (_c, d)) = (oneshot::<i32>(), oneshot::<i32>());
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
