extern crate futures;

use std::sync::mpsc::channel;
use std::thread;

use futures::*;

#[test]
fn and_then1() {
    let (tx, rx) = channel();

    let tx2 = tx.clone();
    let p1 = finished::<_, i32>("a").then(move |t| { tx2.send("first").unwrap(); t });
    let tx2 = tx.clone();
    let p2 = finished("b").then(move |t| { tx2.send("second").unwrap(); t });
    let mut f = p1.and_then(|_| p2);

    assert!(rx.try_recv().is_err());
    f.schedule(move |s| tx.send(s.ok().unwrap()).unwrap());
    assert_eq!(rx.recv(), Ok("first"));
    assert_eq!(rx.recv(), Ok("second"));
    assert_eq!(rx.recv(), Ok("b"));
    assert!(rx.recv().is_err());
}

#[test]
fn and_then2() {
    let (tx, rx) = channel();

    let tx2 = tx.clone();
    let p1 = failed::<i32, _>(2).then(move |t| { tx2.send("first").unwrap(); t });
    let tx2 = tx.clone();
    let p2 = finished("b").then(move |t| { tx2.send("second").unwrap(); t });
    let mut f = p1.and_then(|_| p2);

    assert!(rx.try_recv().is_err());
    f.schedule(move |s| { drop(tx); s.unwrap_err(); });
    assert_eq!(rx.recv(), Ok("first"));
    assert!(rx.recv().is_err());
}

#[test]
fn promise1() {
    let (p, c) = promise::<i32, i32>();
    let t = thread::spawn(|| c.finish(1));

    let (tx, rx) = channel();
    p.map(move |e| tx.send(e).unwrap()).forget();
    assert_eq!(rx.recv(), Ok(1));
    t.join().unwrap();
}

#[test]
fn promise2() {
    let (p, c) = promise::<i32, i32>();
    let t = thread::spawn(|| c.finish(1));
    t.join().unwrap();

    let (tx, rx) = channel();
    p.map(move |e| tx.send(e).unwrap()).forget();
    assert_eq!(rx.recv(), Ok(1));
}

#[test]
fn promise3() {
    let (p, c) = promise::<i32, i32>();
    let (tx, rx) = channel();
    p.map(move |e| tx.send(e).unwrap()).forget();

    let t = thread::spawn(|| c.finish(1));
    t.join().unwrap();

    assert_eq!(rx.recv(), Ok(1));
}

#[test]
fn promise4() {
    let (p, c) = promise::<i32, i32>();
    drop(c);

    let (tx, rx) = channel();
    p.map(move |e| tx.send(e).unwrap()).forget();
    assert!(rx.recv().is_err());
}

#[test]
fn proimse5() {
    let (p, c) = promise::<i32, i32>();
    drop(p);
    c.finish(2);
}

#[test]
fn promise5() {
    let (p, c) = promise::<i32, i32>();
    let t = thread::spawn(|| drop(c));
    let (tx, rx) = channel();
    p.map(move |t| tx.send(t).unwrap()).forget();
    t.join().unwrap();
    assert!(rx.recv().is_err());
}

#[test]
fn cancel1() {
    let (p, c) = promise::<i32, i32>();
    drop(c);
    p.map(|_| panic!()).forget();
}

#[test]
fn map_err1() {
    finished::<i32, i32>(1).map_err(|_| panic!()).forget();
}

#[test]
fn map_err2() {
    let (tx, rx) = channel();
    failed::<i32, i32>(1).map_err(move |v| tx.send(v).unwrap()).forget();
    assert_eq!(rx.recv(), Ok(1));
    assert!(rx.recv().is_err());
}

#[test]
fn map_err3() {
    let (p, c) = promise::<i32, i32>();
    p.map_err(|_| panic!()).forget();
    drop(c);
}

#[test]
fn or_else1() {
    let (p1, c1) = promise::<i32, i32>();
    let (p2, c2) = promise::<i32, i32>();

    let (tx, rx) = channel();
    let tx2 = tx.clone();
    let p1 = p1.map_err(move |i| { tx2.send(i).unwrap(); i });
    let tx2 = tx.clone();
    let p2 = p2.map(move |i| { tx2.send(i).unwrap(); i });

    assert!(rx.try_recv().is_err());
    c1.fail(2);
    c2.finish(3);
    p1.or_else(|_| p2).map(move |v| tx.send(v).unwrap()).forget();

    assert_eq!(rx.recv(), Ok(2));
    assert_eq!(rx.recv(), Ok(3));
    assert_eq!(rx.recv(), Ok(3));
    assert!(rx.recv().is_err());
}

#[test]
fn or_else2() {
    let (p1, c1) = promise::<i32, i32>();

    let (tx, rx) = channel();

    p1.or_else(move |_| {
        tx.send(()).unwrap();
        finished::<i32, i32>(1)
    }).forget();

    c1.finish(2);
    assert!(rx.recv().is_err());
}

#[test]
fn join1() {
    let (tx, rx) = channel();
    finished::<i32, i32>(1).join(finished(2))
                           .map(move |v| tx.send(v).unwrap())
                           .forget();
    assert_eq!(rx.recv(), Ok((1, 2)));
    assert!(rx.recv().is_err());
}

#[test]
fn join2() {
    let (p1, c1) = promise::<i32, i32>();
    let (p2, c2) = promise::<i32, i32>();
    let (tx, rx) = channel();
    p1.join(p2).map(move |v| tx.send(v).unwrap()).forget();
    assert!(rx.try_recv().is_err());
    c1.finish(1);
    assert!(rx.try_recv().is_err());
    c2.finish(2);
    assert_eq!(rx.recv(), Ok((1, 2)));
    assert!(rx.recv().is_err());
}

#[test]
fn join3() {
    let (p1, c1) = promise::<i32, i32>();
    let (p2, c2) = promise::<i32, i32>();
    let (tx, rx) = channel();
    p1.join(p2).map_err(move |v| tx.send(v).unwrap()).forget();
    assert!(rx.try_recv().is_err());
    c1.fail(1);
    assert_eq!(rx.recv(), Ok(1));
    assert!(rx.recv().is_err());
    drop(c2);
}

#[test]
fn join4() {
    let (p1, c1) = promise::<i32, i32>();
    let (p2, c2) = promise::<i32, i32>();
    let (tx, rx) = channel();
    p1.join(p2).map_err(move |v| tx.send(v).unwrap()).forget();
    assert!(rx.try_recv().is_err());
    drop(c1);
    assert!(rx.recv().is_err());
    drop(c2);
}

#[test]
fn join5() {
    let (p1, c1) = promise::<i32, i32>();
    let (p2, c2) = promise::<i32, i32>();
    let (p3, c3) = promise::<i32, i32>();
    let (tx, rx) = channel();
    p1.join(p2).join(p3).map(move |v| tx.send(v).unwrap()).forget();
    assert!(rx.try_recv().is_err());
    c1.finish(1);
    assert!(rx.try_recv().is_err());
    c2.finish(2);
    assert!(rx.try_recv().is_err());
    c3.finish(3);
    assert_eq!(rx.recv(), Ok(((1, 2), 3)));
    assert!(rx.recv().is_err());
}
