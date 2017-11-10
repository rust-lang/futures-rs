extern crate futures;

use futures::future::lazy;
use futures::executor::CurrentThread;

use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering::SeqCst;

#[test]
fn smoke() {
}

#[test]
fn spawning_from_init_future() {
    let cnt = Arc::new(AtomicUsize::new(0));

    CurrentThread::block_with_init(|| {
        let cnt = cnt.clone();

        CurrentThread::spawn(lazy(move || {
            cnt.fetch_add(1, SeqCst);
            Ok(())
        }));
    });

    assert_eq!(1, cnt.load(SeqCst));
}

#[test]
#[should_panic]
fn spawning_out_of_executor_context() {
    CurrentThread::spawn(lazy(|| Ok(())));
}
