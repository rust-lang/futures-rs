#[macro_use]
extern crate futures;

use std::sync::mpsc;
use std::thread;

use futures::{Future, Stream};
use futures::sync::oneshot;
use futures::sync::spsc;

#[test]
fn works() {
    const N: usize = 4;

    let (mut tx, rx) = spsc::channel();

    let (tx2, rx2) = mpsc::channel();
    let (tx3, rx3) = mpsc::channel();
    let t1 = thread::spawn(move || {
        for _ in 0..N+1 {
            let (mytx, myrx) = oneshot::channel();
            tx = tx.send(Ok(myrx)).wait().unwrap();
            tx3.send(mytx).unwrap();
        }
        rx2.recv().unwrap();
        for _ in 0..N {
            let (mytx, myrx) = oneshot::channel();
            tx = tx.send(Ok(myrx)).wait().unwrap();
            tx3.send(mytx).unwrap();
        }
    });

    let (tx4, rx4) = mpsc::channel();
    let t2 = thread::spawn(move || {
        for item in rx.buffer_unordered(N).wait() {
            tx4.send(item.unwrap()).unwrap();
        }
    });

    let o1 = rx3.recv().unwrap();
    let o2 = rx3.recv().unwrap();
    let o3 = rx3.recv().unwrap();
    let o4 = rx3.recv().unwrap();
    assert!(rx4.try_recv().is_err());

    o1.complete(1);
    assert_eq!(rx4.recv(), Ok(1));
    o3.complete(3);
    assert_eq!(rx4.recv(), Ok(3));
    tx2.send(()).unwrap();
    o2.complete(2);
    assert_eq!(rx4.recv(), Ok(2));
    o4.complete(4);
    assert_eq!(rx4.recv(), Ok(4));

    let o5 = rx3.recv().unwrap();
    let o6 = rx3.recv().unwrap();
    let o7 = rx3.recv().unwrap();
    let o8 = rx3.recv().unwrap();
    let o9 = rx3.recv().unwrap();

    o5.complete(5);
    assert_eq!(rx4.recv(), Ok(5));
    o8.complete(8);
    assert_eq!(rx4.recv(), Ok(8));
    o9.complete(9);
    assert_eq!(rx4.recv(), Ok(9));
    o7.complete(7);
    assert_eq!(rx4.recv(), Ok(7));
    o6.complete(6);
    assert_eq!(rx4.recv(), Ok(6));

    t1.join().unwrap();
    t2.join().unwrap();
}
