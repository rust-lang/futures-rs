extern crate futures;
extern crate futures_cpupool;
extern crate futures_executor;

use std::panic::{self, AssertUnwindSafe};
use std::sync::atomic::{AtomicUsize, Ordering, ATOMIC_USIZE_INIT};
use std::thread;
use std::time::Duration;

use futures::future::{Future, FutureExt};
use futures_cpupool::{CpuPool, Builder};
use futures_executor::current_thread::run;

fn done<T: Send + 'static>(t: T) -> Box<Future<Item = T, Error = ()> + Send> {
    Box::new(futures::future::ok(t))
}

#[test]
fn join() {
    let pool = CpuPool::new(2);
    let a = pool.spawn(done(1));
    let b = pool.spawn(done(2));
    let res = run(|c| c.block_on(a.join(b).map(|(a, b)| a + b)));

    assert_eq!(res.unwrap(), 3);
}

#[test]
fn select() {
    let pool = CpuPool::new(2);
    let a = pool.spawn(done(1));
    let b = pool.spawn(done(2));
    let (item1, next) = run(|c| c.block_on(a.select(b))).ok().unwrap();
    let item2 = run(|c| c.block_on(next)).unwrap();

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
    let _handle = pool.spawn_fn(|| {
        FOO.with(|_| ());
        Ok::<(), ()>(())
    });
    drop(pool);

    for _ in 0..100 {
        if CNT.load(Ordering::SeqCst) == 1 {
            return
        }
        thread::sleep(Duration::from_millis(10));
    }
    panic!("thread didn't exit");
}

#[test]
fn lifecycle_test() {
    static SUM_STARTS: AtomicUsize = ATOMIC_USIZE_INIT;
    static SUM_STOPS: AtomicUsize = ATOMIC_USIZE_INIT;
    const EXPECTED_SUM: usize = 3+2+1+0; // 4 threads indexed 3,2,1,0.

    fn after_start(number: usize) {
        SUM_STARTS.fetch_add(number, Ordering::SeqCst);
    }

    fn before_stop(number: usize) {
        SUM_STOPS.fetch_add(number, Ordering::SeqCst);
    }

    let pool = Builder::new()
        .pool_size(4)
        .after_start(after_start)
        .before_stop(before_stop)
        .create();
    let _handle = pool.spawn_fn(|| {
        Ok::<(), ()>(())
    });
    drop(pool);

    for _ in 0..100 {
        if SUM_STOPS.load(Ordering::SeqCst) == EXPECTED_SUM {
            assert_eq!(SUM_STARTS.load(Ordering::SeqCst), EXPECTED_SUM);
            return;
        }
        thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn thread_name() {
    let pool = Builder::new()
        .name_prefix("my-pool-")
        .create();
    let future = pool.spawn_fn(|| {
        assert!(thread::current().name().unwrap().starts_with("my-pool-"));
        Ok::<(), ()>(())
    });
    let _ = run(|c| c.block_on(future));
}

#[test]
fn run_panics() {
    let pool = CpuPool::new(1);
    let future = pool.spawn_fn(|| {
        run(|_| ());
        Ok::<(), ()>(())
    });
    assert!(panic::catch_unwind(AssertUnwindSafe(move || {
        drop(run(|c| c.block_on(future)));
    })).is_err());
}
