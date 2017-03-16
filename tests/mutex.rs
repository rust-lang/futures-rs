extern crate futures;

use futures::sync::Mutex;
use futures::future;
use futures::Future;
use std::thread;
use std::sync::Arc;

#[test]
fn mutually_exclusive() {
    let mutex = Arc::new(Mutex::new(false));

    let mut j = Vec::new();
    for _ in 0..4 {
        let mutex = mutex.clone();
        j.push(thread::spawn(move || for _ in 0..10 {
            let mut g = mutex.lock().wait().unwrap();
            assert!(!*g);
            *g = true;
            *g = false;
        }));
    }

    for i in j {
        i.join().unwrap();
    }
}

#[test]
fn increment() {
    let mutex = Mutex::new(0);

    let mut f = Vec::new();
    for _ in 0..100 {
        f.push(mutex.lock().map(|mut x| *x += 1));
    }

    future::join_all(f).wait().unwrap();

    assert_eq!(*mutex.lock().wait().unwrap(), 100);
}

#[test]
fn single_thread_no_block() {
    let mutex = Mutex::new(());

    for _ in 0..100 {
        assert!(mutex.lock().poll().unwrap().is_ready());
        assert!(mutex.lock().poll().unwrap().is_ready());
        assert!(mutex.lock().poll().unwrap().is_ready());
        let _g = mutex.lock().poll().unwrap();
        assert!(mutex.lock().poll().unwrap().is_not_ready());
        assert!(mutex.lock().poll().unwrap().is_not_ready());
        assert!(mutex.lock().poll().unwrap().is_not_ready());
    }
}

#[test]
fn is_lazy() {
    let mutex = Mutex::new(());

    let _g = mutex.lock();
    let _g = mutex.lock();
    let _g = mutex.lock();
    assert!(mutex.lock().poll().unwrap().is_ready());
    assert!(mutex.lock().poll().unwrap().is_ready());
    assert!(mutex.lock().poll().unwrap().is_ready());
}
