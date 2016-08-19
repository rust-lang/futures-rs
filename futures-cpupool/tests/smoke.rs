extern crate futures;
extern crate futures_cpupool;

use std::sync::atomic::{AtomicUsize, Ordering, ATOMIC_USIZE_INIT};
use std::sync::mpsc::channel;
use std::thread;
use std::time::Duration;

use futures::Future;
use futures_cpupool::CpuPool;

fn get<F>(f: F) -> Result<F::Item, F::Error>
    where F: Future + Send + 'static,
          F::Item: Send,
          F::Error: Send,
{
    let (tx, rx) = channel();
    f.then(move |res| {
        tx.send(res).unwrap();
        futures::finished::<(), ()>(())
    }).forget();
    rx.recv().unwrap()
}

#[test]
fn join() {
    let pool = CpuPool::new(2);
    let a = pool.execute(|| 1);
    let b = pool.execute(|| 2);
    let res = get(a.join(b).map(|(a, b)| a + b));

    assert_eq!(res.unwrap(), 3);
}

#[test]
fn select() {
    let pool = CpuPool::new(2);
    let a = pool.execute(|| 1);
    let b = pool.execute(|| 2);
    let (item1, next) = get(a.select(b)).ok().unwrap();
    let item2 = get(next).unwrap();

    assert!(item1 != item2);
    assert!((item1 == 1 && item2 == 2) || (item1 == 2 && item2 == 1));
}

#[test]
fn threads_go_away() {
    static CNT: AtomicUsize = ATOMIC_USIZE_INIT;

    struct A;

    impl Drop for A {
        fn drop(&mut self) {
            CNT.fetch_add(1, Ordering::SeqCst);
        }
    }

    thread_local!(static FOO: A = A);

    let pool = CpuPool::new(2);
    get(pool.execute(|| {
        FOO.with(|_| ())
    })).unwrap();
    drop(pool);

    for _ in 0..100 {
        if CNT.load(Ordering::SeqCst) == 1 {
            return
        }
        thread::sleep(Duration::from_millis(10));
    }
    panic!("thread didn't exit");
}
