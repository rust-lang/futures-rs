extern crate futures;

use std::thread;
use futures::{oneshot, Future, to_multi};

#[test]
fn two_threads() {
    let (tx, rx) = oneshot::<u32>();
    let mut m = to_multi(rx);
    let f1 = m.future();
    println!("after to_multi");
    let f2 = m.future();

    let (tx1, rx1) = oneshot::<()>();
    let (tx2, rx2) = oneshot::<()>();

    thread::spawn(move || {
        assert!(*f1.wait().unwrap().unwrap() == 3);
        println!("after f1.wait()");
        tx1.complete(());
    });
    thread::spawn(move || {
        assert!(*f2.wait().unwrap().unwrap() == 3);
        println!("after f2.wait()");
        tx2.complete(());
    });

    thread::spawn(|| {
        println!("after spawn");
        tx.complete(3);
    });
    println!("after tx.complete()");
    m.wait().unwrap();
    println!("after m.wait()");
    rx1.wait().unwrap();
    rx2.wait().unwrap();
}
