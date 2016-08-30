extern crate futures;

use futures::{failed, finished, Future, oneshot, Poll};
use futures::stream::*;

mod support;
use support::*;


fn list() -> Receiver<i32, u32> {
    let (tx, rx) = channel();
    tx.send(Ok(1))
      .and_then(|tx| tx.send(Ok(2)))
      .and_then(|tx| tx.send(Ok(3)))
      .forget();
    return rx
}

fn err_list() -> Receiver<i32, u32> {
    let (tx, rx) = channel();
    tx.send(Ok(1))
      .and_then(|tx| tx.send(Ok(2)))
      .and_then(|tx| tx.send(Err(3)))
      .forget();
    return rx
}

#[test]
fn map() {
    assert_done(|| list().map(|a| a + 1).collect(), Ok(vec![2, 3, 4]));
}

#[test]
fn map_err() {
    assert_done(|| err_list().map_err(|a| a + 1).collect(), Err(4));
}

#[test]
fn fold() {
    assert_done(|| list().fold(0, |a, b| finished::<i32, u32>(a + b)), Ok(6));
    assert_done(|| err_list().fold(0, |a, b| finished::<i32, u32>(a + b)), Err(3));
}

#[test]
fn filter() {
    assert_done(|| list().filter(|a| *a % 2 == 0).collect(), Ok(vec![2]));
}

#[test]
fn filter_map() {
    assert_done(|| list().filter_map(|x| {
        if x % 2 == 0 {
            Some(x + 10)
        } else {
            None
        }
    }).collect(), Ok(vec![12]));
}

#[test]
fn and_then() {
    assert_done(|| list().and_then(|a| Ok(a + 1)).collect(), Ok(vec![2, 3, 4]));
    assert_done(|| list().and_then(|a| failed::<i32, u32>(a as u32)).collect(),
                Err(1));
}

#[test]
fn then() {
    assert_done(|| list().then(|a| a.map(|e| e + 1)).collect(), Ok(vec![2, 3, 4]));

}

#[test]
fn or_else() {
    assert_done(|| err_list().or_else(|a| {
        finished::<i32, u32>(a as i32)
    }).collect(), Ok(vec![1, 2, 3]));
}

#[test]
fn flatten() {
    assert_done(|| list().map(|_| list()).flatten().collect(),
                Ok(vec![1, 2, 3, 1, 2, 3, 1, 2, 3]));

}

#[test]
fn skip() {
    assert_done(|| list().skip(2).collect(), Ok(vec![3]));
}

#[test]
fn skip_while() {
    assert_done(|| list().skip_while(|e| Ok(*e % 2 == 1)).collect(),
                Ok(vec![2, 3]));
}
#[test]
fn take() {
    assert_done(|| list().take(2).collect(), Ok(vec![1, 2]));
}

#[test]
fn peekable() {
    assert_done(|| list().peekable().collect(), Ok(vec![1, 2, 3]));
}

#[test]
fn fuse() {
    let mut stream = list().fuse();
    futures::task::ThreadTask::new().enter(|| {
        assert_eq!(stream.poll(), Poll::Ok(Some(1)));
        assert_eq!(stream.poll(), Poll::Ok(Some(2)));
        assert_eq!(stream.poll(), Poll::Ok(Some(3)));
        assert_eq!(stream.poll(), Poll::Ok(None));
        assert_eq!(stream.poll(), Poll::Ok(None));
        assert_eq!(stream.poll(), Poll::Ok(None));
    });
}

#[test]
fn buffered() {
    let (tx, rx) = channel::<_, u32>();
    let (a, b) = oneshot::<u32>();
    let (c, d) = oneshot::<u32>();

    tx.send(Ok(b.map_err(|_| 2).boxed()))
      .and_then(|tx| tx.send(Ok(d.map_err(|_| 4).boxed())))
      .forget();

    let mut rx = rx.buffered(2);
    sassert_empty(&mut rx);
    c.complete(3);
    sassert_empty(&mut rx);
    a.complete(5);
    sassert_next(&mut rx, 5);
    sassert_next(&mut rx, 3);
    sassert_done(&mut rx);

    let (tx, rx) = channel::<_, u32>();
    let (a, b) = oneshot::<u32>();
    let (c, d) = oneshot::<u32>();

    tx.send(Ok(b.map_err(|_| 2).boxed()))
      .and_then(|tx| tx.send(Ok(d.map_err(|_| 4).boxed())))
      .forget();

    let mut rx = rx.buffered(1);
    sassert_empty(&mut rx);
    c.complete(3);
    sassert_empty(&mut rx);
    a.complete(5);
    sassert_next(&mut rx, 5);
    sassert_next(&mut rx, 3);
    sassert_done(&mut rx);
}

#[test]
fn zip() {
    assert_done(|| list().zip(list()).collect(),
                Ok(vec![(1, 1), (2, 2), (3, 3)]));
    assert_done(|| list().zip(list().take(2)).collect(),
                Ok(vec![(1, 1), (2, 2)]));
    assert_done(|| list().take(2).zip(list()).collect(),
                Ok(vec![(1, 1), (2, 2)]));
    assert_done(|| err_list().zip(list()).collect(), Err(3));
    assert_done(|| list().zip(list().map(|x| x + 1)).collect(),
                Ok(vec![(1, 2), (2, 3), (3, 4)]));
}

#[test]
fn peek() {
    let mut peekable = list().peekable();
    assert_eq!(peekable.peek().unwrap(), Ok(Some(&1)));
    assert_eq!(peekable.peek().unwrap(), Ok(Some(&1)));
    assert_eq!(peekable.poll().unwrap(), Ok(Some(1)));
}

#[test]
fn wait() {
    assert_eq!(list().wait().collect::<Result<Vec<_>, _>>(),
               Ok(vec![1, 2, 3]));
}
