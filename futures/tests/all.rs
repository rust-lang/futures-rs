use futures::channel::oneshot::{self, Canceled};
use futures::executor::block_on;
use futures::future::{
    self, err, join, ok, try_join, try_select, Either, Future, FutureExt, TryFutureExt,
};
use futures::task::Poll;
use futures_test::task::{noop_context, panic_context};
use std::fmt;
use std::sync::mpsc::{channel, TryRecvError};

fn assert_done<T, F, Fut>(actual_fut: F, expected: T)
where
    T: PartialEq + fmt::Debug,
    F: FnOnce() -> Fut,
    Fut: Future<Output = T> + Unpin,
{
    let output = block_on(actual_fut());
    assert_eq!(output, expected);
}

fn assert_pending<Fut, F>(mut f: F)
where
    F: FnMut() -> Fut,
    Fut: Future + Unpin,
{
    assert!(f().poll_unpin(&mut panic_context()).is_pending());
}

fn unwrap<T, E: fmt::Debug>(x: Poll<Result<T, E>>) -> T {
    match x {
        Poll::Ready(Ok(x)) => x,
        _ => panic!(),
    }
}

fn unwrap_err<T, E: fmt::Debug>(x: Poll<Result<T, E>>) -> E {
    match x {
        Poll::Ready(Err(x)) => x,
        _ => panic!(),
    }
}

fn f_ok(a: i32) -> future::Ready<Result<i32, u32>> {
    ok(a)
}
fn f_err(a: u32) -> future::Ready<Result<i32, u32>> {
    err(a)
}
fn r_ok(a: i32) -> Result<i32, u32> {
    Ok(a)
}
fn r_err(a: u32) -> Result<i32, u32> {
    Err(a)
}

fn unselect<T, E, A, B>(
    r: Result<Either<(T, B), (T, A)>, Either<(E, B), (E, A)>>,
) -> future::Ready<Result<T, E>> {
    match r {
        Ok(Either::Left((t, _))) | Ok(Either::Right((t, _))) => ok(t),
        Err(Either::Left((e, _))) | Err(Either::Right((e, _))) => err(e),
    }
}

#[test]
fn result_smoke() {
    fn is_future_v<A, B, C>(_: C)
    where
        A: Send + 'static,
        B: Send + 'static,
        C: Future<Output = Result<A, B>>,
    {
    }

    is_future_v::<i32, u32, _>(f_ok(1).map_ok(|a| a + 1));
    is_future_v::<i32, u32, _>(f_ok(1).map_err(|a| a + 1));
    is_future_v::<i32, u32, _>(f_ok(1).and_then(ok));
    is_future_v::<i32, u32, _>(f_ok(1).or_else(err));
    is_future_v::<(i32, i32), u32, _>(try_join(f_ok(1), err(3)));
    // is_future_v::<i32, u32, _>(f_ok(1).map_ok(f_ok).flatten());

    assert_done(|| f_ok(1), r_ok(1));
    assert_done(|| f_err(1), r_err(1));
    assert_done(|| future::ready(Ok(1)), r_ok(1));
    assert_done(|| future::ready(Err(1)), r_err(1));
    assert_done(|| ok(1), r_ok(1));
    assert_done(|| err(1), r_err(1));
    assert_done(|| f_ok(1).map_ok(|a| a + 2), r_ok(3));
    assert_done(|| f_err(1).map_ok(|a| a + 2), r_err(1));
    assert_done(|| f_ok(1).map_err(|a| a + 2), r_ok(1));
    assert_done(|| f_err(1).map_err(|a| a + 2), r_err(3));
    assert_done(|| f_ok(1).and_then(|a| ok(a + 2)), r_ok(3));
    assert_done(|| f_err(1).and_then(|a| ok(a + 2)), r_err(1));
    assert_done(|| f_ok(1).and_then(|a| err(a as u32 + 3)), r_err(4));
    assert_done(|| f_err(1).and_then(|a| err(a as u32 + 4)), r_err(1));
    assert_done(|| f_ok(1).or_else(|a| ok(a as i32 + 2)), r_ok(1));
    assert_done(|| f_err(1).or_else(|a| ok(a as i32 + 2)), r_ok(3));
    assert_done(|| f_ok(1).or_else(|a| err(a + 3)), r_ok(1));
    assert_done(|| f_err(1).or_else(|a| err(a + 4)), r_err(5));
    assert_done(|| try_select(f_ok(1), f_err(2)).then(unselect), r_ok(1));
    assert_done(|| try_select(f_ok(1), ok(2)).then(unselect), r_ok(1));
    assert_done(|| try_select(f_err(1), f_ok(1)).then(unselect), r_err(1));
    assert_done(
        || try_select(f_ok(1), future::pending()).then(unselect),
        Ok(1),
    );
    assert_done(
        || try_select(future::pending(), f_ok(1)).then(unselect),
        Ok(1),
    );
    assert_done(|| try_join(f_ok(1), f_err(1)), Err(1));
    assert_done(|| try_join(f_ok(1), ok(2)), Ok((1, 2)));
    assert_done(|| try_join(f_err(1), f_ok(1)), Err(1));
    assert_done(|| f_ok(1).then(|_| ok(2)), r_ok(2));
    assert_done(|| f_ok(1).then(|_| err(2)), r_err(2));
    assert_done(|| f_err(1).then(|_| ok(2)), r_ok(2));
    assert_done(|| f_err(1).then(|_| err(2)), r_err(2));
}

#[test]
fn test_pending() {
    fn pending() -> future::Pending<Result<i32, u32>> {
        future::pending()
    }

    assert_pending(|| pending());
    assert_pending(|| try_select(pending(), pending()));
    assert_pending(|| join(pending(), pending()));
    assert_pending(|| join(pending(), f_ok(1)));
    assert_pending(|| join(f_ok(1), pending()));
    assert_pending(|| pending().or_else(move |_| pending()));
    assert_pending(|| pending().and_then(move |_| pending()));
    assert_pending(|| f_err(1).or_else(move |_| pending()));
    assert_pending(|| f_ok(1).and_then(move |_| pending()));
    assert_pending(|| pending().map_ok(|a| a + 1));
    assert_pending(|| pending().map_err(|a| a + 1));
    assert_pending(|| pending().then(|a| future::ready(a)));
}

#[test]
fn test_ok() {
    assert_done(|| ok(1), r_ok(1));
    assert_done(|| err(1), r_err(1));
}

/* TODO: TryFutureExt::try_flatten
#[test]
fn flatten() {
    fn ok<T: Send + 'static, E>(a: T) -> future::Ready<Result<T, E>> {
        future::ok(a)
    }
    fn err<E: Send + 'static, T>(b: E) -> future::Ready<Result<T, E>> {
        future::err(b)
    }

    assert_done(|| ok(ok(1)).flatten(), r_ok(1));
    assert_done(|| ok(err(1)).flatten(), r_err(1));
    assert_done(|| err(1u32).map_ok(ok).flatten(), r_err(1));
    assert_done(|| ok(ok(1)).flatten(), r_ok(1));
    assert_pending(|| ok(future::pending::<Result<i32, u32>>()).flatten());
    assert_pending(|| future::pending::<Result<i32, u32>>().map_ok(ok).flatten());
}
*/

#[test]
fn smoke_oneshot() {
    assert_done(
        || {
            let (c, p) = oneshot::channel();
            c.send(1).unwrap();
            p
        },
        Ok(1),
    );
    assert_done(
        || {
            let (c, p) = oneshot::channel::<i32>();
            drop(c);
            p
        },
        Err(Canceled),
    );
    let mut completes = Vec::new();
    assert_pending(|| {
        let (a, b) = oneshot::channel::<i32>();
        completes.push(a);
        b
    });

    let (c, mut p) = oneshot::channel::<i32>();
    drop(c);
    unwrap_err(p.poll_unpin(&mut panic_context()));
    let (c, p) = oneshot::channel::<i32>();
    drop(c);
    let (tx, rx) = channel();
    p.then(move |_| tx.send(())); //.forget();
    rx.recv().unwrap();
}

#[test]
fn select_cancels() {
    let ((a, b), (c, d)) = (oneshot::channel::<i32>(), oneshot::channel::<i32>());
    let ((btx, brx), (dtx, drx)) = (channel(), channel());
    let b = b.map_ok(move |b| {
        btx.send(b).unwrap();
        b
    });
    let d = d.map_ok(move |d| {
        dtx.send(d).unwrap();
        d
    });

    let mut f = try_select(b, d).then(unselect);
    // assert!(f.poll(&mut Task::new()).is_pending());
    assert!(brx.try_recv().is_err());
    assert!(drx.try_recv().is_err());
    a.send(1).unwrap();
    let cx = &mut noop_context();
    unwrap(f.poll_unpin(cx));
    assert_eq!(brx.recv().unwrap(), 1);
    drop(c);
    assert!(drx.recv().is_err());

    let ((a, b), (c, d)) = (oneshot::channel::<i32>(), oneshot::channel::<i32>());
    let ((btx, _brx), (dtx, drx)) = (channel(), channel());
    let b = b.map_ok(move |b| {
        btx.send(b).unwrap();
        b
    });
    let d = d.map_ok(move |d| {
        dtx.send(d).unwrap();
        d
    });

    let mut f = try_select(b, d).then(unselect);
    assert!(f.poll_unpin(cx).is_pending());
    assert!(f.poll_unpin(cx).is_pending());
    a.send(1).unwrap();
    unwrap(f.poll_unpin(cx));
    drop((c, f));
    assert!(drx.recv().is_err());
}

#[test]
fn join_cancels() {
    let ((a, b), (c, d)) = (oneshot::channel::<i32>(), oneshot::channel::<i32>());
    let ((btx, _brx), (dtx, drx)) = (channel(), channel());
    let b = b.map(move |b| {
        btx.send(b).unwrap();
        b
    });
    let d = d.map(move |d| {
        dtx.send(d).unwrap();
        d
    });

    let mut f = try_join(b, d);
    drop(a);
    unwrap_err(f.poll_unpin(&mut panic_context()));
    drop(c);
    assert!(drx.recv().is_err());

    let ((a, b), (c, d)) = (oneshot::channel::<i32>(), oneshot::channel::<i32>());
    let ((btx, _brx), (dtx, drx)) = (channel(), channel());
    let b = b.map(move |b| {
        btx.send(b).unwrap();
        b
    });
    let d = d.map(move |d| {
        dtx.send(d).unwrap();
        d
    });

    let (tx, rx) = channel();
    let f = try_join(b, d);
    f.then(move |_| {
        tx.send(()).unwrap();
        ok(())
    }); // .forget();
    assert!(rx.try_recv().is_err());
    drop(a);
    rx.recv().unwrap();
    drop(c);
    assert!(drx.recv().is_err());
}

#[test]
fn join_incomplete() {
    let (a, b) = oneshot::channel::<i32>();
    let (tx, rx) = channel();
    let cx = &mut noop_context();
    let mut f = try_join(ok(1), b).map_ok(move |r| tx.send(r).unwrap());
    assert!(f.poll_unpin(cx).is_pending());
    assert!(rx.try_recv().is_err());
    a.send(2).unwrap();
    unwrap(f.poll_unpin(cx));
    assert_eq!(rx.recv().unwrap(), (1, 2));

    let (a, b) = oneshot::channel::<i32>();
    let (tx, rx) = channel();
    let mut f = try_join(b, ok(2)).map_ok(move |r| tx.send(r).unwrap());
    assert!(f.poll_unpin(cx).is_pending());
    assert!(rx.try_recv().is_err());
    a.send(1).unwrap();
    unwrap(f.poll_unpin(cx));
    assert_eq!(rx.recv().unwrap(), (1, 2));

    let (a, b) = oneshot::channel::<i32>();
    let (tx, rx) = channel();
    let mut f = try_join(ok(1), b).map_err(move |_r| tx.send(2).unwrap());
    assert!(f.poll_unpin(cx).is_pending());
    assert!(rx.try_recv().is_err());
    drop(a);
    unwrap_err(f.poll_unpin(cx));
    assert_eq!(rx.recv().unwrap(), 2);

    let (a, b) = oneshot::channel::<i32>();
    let (tx, rx) = channel();
    let mut f = try_join(b, ok(2)).map_err(move |_r| tx.send(1).unwrap());
    assert!(f.poll_unpin(cx).is_pending());
    assert!(rx.try_recv().is_err());
    drop(a);
    unwrap_err(f.poll_unpin(cx));
    assert_eq!(rx.recv().unwrap(), 1);
}

#[test]
fn select() {
    assert_done(
        || try_select(f_ok(2), future::pending()).then(unselect),
        Ok(2),
    );
    assert_done(
        || try_select(future::pending(), f_ok(2)).then(unselect),
        Ok(2),
    );
    assert_done(
        || try_select(f_err(2), future::pending()).then(unselect),
        Err(2),
    );
    assert_done(
        || try_select(future::pending(), f_err(2)).then(unselect),
        Err(2),
    );

    assert_done(
        || {
            try_select(f_ok(1), f_ok(2))
                .map_err(|_| 0)
                .and_then(|either_tup| {
                    let (a, b) = either_tup.into_inner();
                    b.map_ok(move |b| a + b)
                })
        },
        Ok(3),
    );

    // Finish one half of a select and then fail the second, ensuring that we
    // get the notification of the second one.
    {
        let ((a, b), (c, d)) = (oneshot::channel::<i32>(), oneshot::channel::<i32>());
        let f = try_select(b, d);
        let (tx, rx) = channel();
      block_on(  f.map_ok(move |r| tx.send(r).unwrap()));
        a.send(1).unwrap();
        let val = rx.recv().unwrap();
        assert_eq!(val, 1);
        let (tx, rx) = channel();
        block_on(rx.map_err(move |_r| tx.send(2).unwrap()));
        assert_eq!(rx.try_recv().err().unwrap(), TryRecvError::Empty);
        drop(c);
        assert_eq!(rx.recv().unwrap(), 2);
    }

    // Fail the second half and ensure that we see the first one finish
    {
        let ((a, b), (c, d)) = (oneshot::channel::<i32>(), oneshot::channel::<i32>());
        let f = try_select(b, d);
        let (tx, rx) = channel();
        block_on(f.map_err(move |r| tx.send((1, r.into_inner().1)).unwrap()));
        drop(c);
        assert_eq!(rx.recv().unwrap(), 1);
        let (tx2, rx2) = channel();
        block_on(rx.map_ok(move |r| tx2.send(r).unwrap()));
        assert_eq!(rx2.try_recv().err().unwrap(), TryRecvError::Empty);
        a.send(2).unwrap();
        assert_eq!(rx2.recv().unwrap(), 2);
    }

    // Cancelling the first half should cancel the second
    {
        let ((_a, b), (_c, d)) = (oneshot::channel::<i32>(), oneshot::channel::<i32>());
        let ((btx, brx), (dtx, drx)) = (channel(), channel());
        let b = b.map_ok(move |v| {
            btx.send(v).unwrap();
            v
        });
        let d = d.map_ok(move |v| {
            dtx.send(v).unwrap();
            v
        });
        let f = try_select(b, d);
        drop(f);
        assert!(drx.recv().is_err());
        assert!(brx.recv().is_err());
    }

    // Cancel after a schedule
    {
        let ((_a, b), (_c, d)) = (oneshot::channel::<i32>(), oneshot::channel::<i32>());
        let ((btx, brx), (dtx, drx)) = (channel(), channel());
        let b = b.map_ok(move |v| {
            btx.send(v).unwrap();
            v
        });
        let d = d.map_ok(move |v| {
            dtx.send(v).unwrap();
            v
        });
        let mut f = try_select(b, d);
        let _res = f.poll_unpin(&mut noop_context());
        drop(f);
        assert!(drx.recv().is_err());
        assert!(brx.recv().is_err());
    }

    // Cancel propagates
    {
        let ((a, b), (_c, d)) = (oneshot::channel::<i32>(), oneshot::channel::<i32>());
        let ((btx, brx), (dtx, drx)) = (channel(), channel());
        let b = b.map_ok(move |v| {
            btx.send(v).unwrap();
            v
        });
        let d = d.map_ok(move |v| {
            dtx.send(v).unwrap();
            v
        });
        let (tx, rx) = channel();
        try_select(b, d).map_ok(move |_| tx.send(()).unwrap()); //.forget();
        drop(a);
        assert!(drx.recv().is_err());
        assert!(brx.recv().is_err());
        assert!(rx.recv().is_err());
    }

    // Cancel on early drop
    {
        let (tx, rx) = channel();
        let f = try_select(
            f_ok(1),
            future::pending::<Result<_, ()>>().map_ok(move |()| {
                tx.send(()).unwrap();
                1
            }),
        );
        drop(f);
        assert!(rx.recv().is_err());
    }
}
