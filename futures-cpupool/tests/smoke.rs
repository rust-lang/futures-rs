extern crate futures;
extern crate futures_cpupool;

use std::sync::atomic::{AtomicUsize, Ordering, ATOMIC_USIZE_INIT};
use std::sync::mpsc::channel;
use std::thread;
use std::time::Duration;

use futures::{Future, BoxFuture};
use futures::task::{Executor, Task, Run};
use futures_cpupool::CpuPool;

// TODO: centralize this?
pub trait ForgetExt {
    fn forget(self);
}

impl<F> ForgetExt for F
    where F: Future + Sized + Send + 'static,
          F::Item: Send,
          F::Error: Send
{
    fn forget(self) {
        use std::sync::Arc;

        struct ForgetExec;

        impl Executor for ForgetExec {
            fn execute(&self, run: Run) {
                run.run()
            }
        }

        Task::new(Arc::new(ForgetExec), self.then(|_| Ok(())).boxed()).unpark();
    }
}


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

fn done<T: Send + 'static>(t: T) -> BoxFuture<T, ()> {
    futures::done(Ok(t)).boxed()
}

#[test]
fn join() {
    let pool = CpuPool::new(2);
    let a = pool.spawn(done(1));
    let b = pool.spawn(done(2));
    let res = get(a.join(b).map(|(a, b)| a + b));

    assert_eq!(res.unwrap(), 3);
}

#[test]
fn select() {
    let pool = CpuPool::new(2);
    let a = pool.spawn(done(1));
    let b = pool.spawn(done(2));
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
    get(pool.spawn(futures::lazy(|| {
        FOO.with(|_| ());
        let res: Result<(), ()> = Ok(());
        res
    }))).unwrap();
    drop(pool);

    for _ in 0..100 {
        if CNT.load(Ordering::SeqCst) == 1 {
            return
        }
        thread::sleep(Duration::from_millis(10));
    }
    panic!("thread didn't exit");
}
