extern crate futures;
extern crate futures_cpupool;

use std::sync::atomic::{AtomicUsize, Ordering, ATOMIC_USIZE_INIT};
use std::thread;
use std::time::Duration;

use futures::{Future, BoxFuture};
use futures_cpupool::CpuPool;

fn done<T: Send + 'static>(t: T) -> BoxFuture<T, ()> {
    futures::done(Ok(t)).boxed()
}

#[test]
fn join() {
    let pool = CpuPool::new(2);
    let a = pool.spawn(done(1));
    let b = pool.spawn(done(2));
    let res = a.join(b).map(|(a, b)| a + b).wait();

    assert_eq!(res.unwrap(), 3);
}

#[test]
fn select() {
    let pool = CpuPool::new(2);
    let a = pool.spawn(done(1));
    let b = pool.spawn(done(2));
    let (item1, next) = a.select(b).wait().ok().unwrap();
    let item2 = next.wait().unwrap();

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
    let _handle = pool.spawn(futures::lazy(|| {
        FOO.with(|_| ());
        Ok::<(), ()>(())
    }));
    drop(pool);

    for _ in 0..100 {
        if CNT.load(Ordering::SeqCst) == 1 {
            return
        }
        thread::sleep(Duration::from_millis(10));
    }
    panic!("thread didn't exit");
}
