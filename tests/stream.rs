extern crate futures;

use std::thread;

use futures::channel::{channel, Receiver};
use futures::*;
use futures::stream::{Stream, PollError};
use futures::bufstream::bufstream;

#[test]
fn smoke() {
    let (tx, rx) = channel::<i32, u32>();
    tx.send(Ok(1))
      .and_then(|tx| tx.send(Ok(2)))
      .and_then(|tx| tx.send(Ok(3)))
      .schedule(|r| assert!(r.is_ok()));
    assert_eq!(rx.collect().await(), Ok(vec![1, 2, 3]));

    let (tx, rx) = channel::<i32, u32>();
    tx.send(Ok(1))
      .and_then(|tx| tx.send(Err(2)))
      .and_then(|tx| tx.send(Ok(3)))
      .schedule(|r| assert!(r.is_ok()));
    assert_eq!(rx.collect().await(), Err(2));
}

fn list() -> Receiver<i32, u32> {
    let (tx, rx) = channel();
    tx.send(Ok(1))
      .and_then(|tx| tx.send(Ok(2)))
      .and_then(|tx| tx.send(Ok(3)))
      .schedule(|r| assert!(r.is_ok()));
    return rx
}

fn err_list() -> Receiver<i32, u32> {
    let (tx, rx) = channel();
    tx.send(Ok(1))
      .and_then(|tx| tx.send(Ok(2)))
      .and_then(|tx| tx.send(Err(3)))
      .schedule(|_| {});
    return rx
}

fn collect_poll<S: Stream>(mut s: S) -> Result<Vec<S::Item>, S::Error> {
    let mut base = Vec::new();
    loop {
        match s.poll() {
            Ok(item) => base.push(item),
            Err(PollError::Empty) => return Ok(base),
            Err(PollError::Other(e)) => return Err(e),
            Err(PollError::NotReady) => panic!("blocked?"),
        }
    }
}

#[test]
fn adapters() {
    assert_eq!(list().map(|a| a + 1).collect().await(), Ok(vec![2, 3, 4]));
    assert_eq!(err_list().map_err(|a| a + 1).collect().await(), Err(4));
    assert_eq!(list().fold(0, |a, b| a + b).await(), Ok(6));
    assert_eq!(list().filter(|a| *a % 2 == 0).collect().await(), Ok(vec![2]));
    assert_eq!(list().and_then(|a| Ok(a + 1)).collect().await(),
               Ok(vec![2, 3, 4]));
    assert_eq!(err_list().or_else(|a| {
        finished::<i32, u32>(a as i32)
    }).collect().await(), Ok(vec![1, 2, 3]));
    assert_eq!(list().map(|_| list()).flat_map().collect().await(),
               Ok(vec![1, 2, 3, 1, 2, 3, 1, 2, 3]));
    assert_eq!(list().map(|i| finished::<_, u32>(i)).flatten().collect().await(),
               Ok(vec![1, 2, 3]));
}

#[test]
fn adapters_poll() {
    assert_eq!(collect_poll(list().map(|a| a + 1)), Ok(vec![2, 3, 4]));
    assert_eq!(collect_poll(err_list().map_err(|a| a + 1)), Err(4));
    assert_eq!(collect_poll(list().filter(|a| *a % 2 == 0)), Ok(vec![2]));
    assert_eq!(collect_poll(list().and_then(|a| Ok(a + 1))), Ok(vec![2, 3, 4]));
    assert_eq!(collect_poll(err_list().and_then(|a| Ok(a + 1))), Err(3));
    assert_eq!(collect_poll(err_list().and_then(|a| {
        failed::<i32, _>(a as u32)
    })), Err(1));
    assert_eq!(collect_poll(err_list().or_else(|a| finished::<_, u32>(a as i32))),
               Ok(vec![1, 2, 3]));

    let (tx, rx) = channel::<i32, u32>();
    let (rx2, tx2) = promise::pair();
    let mut rx2 = Some(rx2);
    let mut rx = rx.and_then(move |_a| rx2.take().unwrap());
    match rx.poll() {
        Err(PollError::NotReady) => {}
        _ => panic!("ready?"),
    }
    tx.send(Ok(1)).schedule(|_| ());
    match rx.poll() {
        Err(PollError::NotReady) => {}
        _ => panic!("ready?"),
    }
    match rx.poll() {
        Err(PollError::NotReady) => {}
        _ => panic!("ready?"),
    }
    tx2.finish(1);
    match rx.poll() {
        Ok(1) => {},
        Err(PollError::NotReady) => panic!("not ready?"),
        Err(PollError::Empty) => panic!("empty?"),
        _ => panic!("not ready?"),
    }

    // let (tx, rx) = channel::<i32, u32>();
    // let rx = rx.and_then(|a| failed::<i32, _>(a as u32));
    // tx.send(Ok(1)).schedule(|_| ());
    // assert_eq!(rx.collect().await(), Err(1));
    // assert_eq!(list().fold(0, |a, b| a + b).await(), Ok(6));
    // assert_eq!(list().and_then(|a| Ok(a + 1)).collect().await(),
    //            Ok(vec![2, 3, 4]));
    // assert_eq!(err_list().or_else(|a| {
    //     finished::<i32, u32>(a as i32)
    // }).collect().await(), Ok(vec![1, 2, 3]));
    // assert_eq!(list().map(|_| list()).flat_map().collect().await(),
    //            Ok(vec![1, 2, 3, 1, 2, 3, 1, 2, 3]));
    // assert_eq!(list().map(|i| finished::<_, u32>(i)).flatten().collect().await(),
    //            Ok(vec![1, 2, 3]));

    assert_eq!(list().collect().poll().ok().unwrap(), Ok(vec![1, 2, 3]));
    assert_eq!(err_list().collect().poll().ok().unwrap(), Err(3));
    assert_eq!(list().fold(0, |a, b| a + b).poll().ok().unwrap(), Ok(6));
    assert_eq!(err_list().fold(0, |a, b| a + b).poll().ok().unwrap(), Err(3));
    assert_eq!(list().map(|a| finished::<_, u32>(a))
                     .flatten().collect().poll().ok().unwrap(),
               Ok(vec![1, 2, 3]));
    assert_eq!(list().map(|_a| list()).flat_map()
                     .collect().poll().ok().unwrap(),
               Ok(vec![1, 2, 3, 1, 2, 3, 1, 2, 3]));
}

#[test]
fn rxdrop() {
    let (tx, rx) = channel::<i32, u32>();
    drop(rx);
    assert!(tx.send(Ok(1)).await().is_err());
}

#[test]
fn bufstream_smoke() {
    let (tx, mut rx) = bufstream::<i32, u32>(4);
    let (vrx, mut vtx): (Vec<_>, Vec<_>) = (0..4).map(|_| {
        let (a, b) = promise::pair::<i32, u32>();
        (a, Some(b))
    }).unzip();
    for (a, b) in tx.zip(vrx) {
        b.schedule(|val| a.send(val));
    }

    assert_eq!(rx.poll(), Err(PollError::NotReady));
    vtx[0].take().unwrap().finish(2);
    assert_eq!(rx.poll(), Ok(2));
    assert_eq!(rx.poll(), Err(PollError::NotReady));
    vtx[3].take().unwrap().finish(4);
    assert_eq!(rx.poll(), Ok(4));
    assert_eq!(rx.poll(), Err(PollError::NotReady));
    vtx[1].take().unwrap().fail(3);
    assert_eq!(rx.poll(), Err(PollError::Other(3)));
    assert_eq!(rx.poll(), Err(PollError::NotReady));
    vtx[2].take().unwrap().finish(1);
    assert_eq!(rx.poll(), Ok(1));
    assert_eq!(rx.poll(), Err(PollError::Empty));
}

#[test]
fn bufstream_concurrent() {
    let (tx, rx) = bufstream::<i32, u32>(4);
    let (vrx, vtx): (Vec<_>, Vec<_>) = (0..4).map(|_| {
        promise::pair::<i32, u32>()
    }).unzip();
    for (a, b) in tx.zip(vrx) {
        b.schedule(|val| a.send(val));
    }

    let t = thread::spawn(|| {
        let mut it = vtx.into_iter();
        it.next().unwrap().finish(2);
        it.next_back().unwrap().finish(4);
        it.next().unwrap().finish(3);
        it.next_back().unwrap().finish(1);
        assert!(it.next().is_none());
    });

    assert_eq!(rx.collect().await(), Ok(vec![2, 4, 3, 1]));
    t.join().unwrap();
}
