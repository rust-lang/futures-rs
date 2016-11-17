extern crate futures;

use std::thread;

use futures::Stream;

#[test]
fn smoke() {
    const N: usize = 1_000_000;

    let (mut tx, rx) = futures::sync::spsc::unbounded::<usize>();

    let t = thread::spawn(move || {
        for i in 0..N {
            tx.send(i).unwrap();
        }
    });

    let rx = rx.wait();

    let mut i = 0;
    for msg in rx {
        assert_eq!(msg.unwrap(), i);
        i += 1;
    }

    t.join().unwrap();
}

#[test]
fn drop_pending() {
    struct A;

    static mut HIT: bool = false;

    impl Drop for A {
        fn drop(&mut self) {
            unsafe { HIT = true; }
        }
    }

    let (mut tx, rx) = futures::sync::spsc::unbounded();
    tx.send(A).unwrap();
    drop(rx);

    unsafe { assert!(HIT); }
}
