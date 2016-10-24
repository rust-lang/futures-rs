extern crate futures;

use std::thread;
use futures::{oneshot, Future};

#[test]
fn two_threads() {
    let (tx, rx) = oneshot::<u32>();
    let f1 = rx.shared();
    println!("after shared()");
    let f2 = f1.clone();

    let (tx1, rx1) = oneshot::<()>();
    let (tx2, rx2) = oneshot::<()>();

    thread::spawn(move || {
        assert!(*f1.wait().unwrap() == 3);
        println!("after f1.wait()");
        tx1.complete(());
    });
    thread::spawn(move || {
        assert!(*f2.wait().unwrap() == 3);
        println!("after f2.wait()");
        tx2.complete(());
    });

    thread::spawn(|| {
        println!("after spawn");
        tx.complete(3);
    });

    rx1.wait().unwrap();
    rx2.wait().unwrap();
    println!("done!");
}
