#![feature(async_await)]
use futures::future::{FutureExt, TryFutureExt};
use futures::compat::Executor01CompatExt;
use tokio_threadpool::ThreadPool;

#[test]
fn foo () {
    let pool01 = ThreadPool::new();
    let pool03 = pool01.compat();

    let future03 = async {
        println!("Running on the pool");
    };

    pool01.spawn(future03.unit_error().boxed().compat(pool01));

    pool01.shutdown().wait().unwrap();
}
