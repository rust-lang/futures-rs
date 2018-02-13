#![feature(test)]

extern crate futures;
extern crate test;

use futures::prelude::*;
use futures::task::{self, Waker, Notify, NotifyHandle, LocalMap};

use test::Bencher;

fn notify_noop() -> NotifyHandle {
    struct Noop;

    impl Notify for Noop {
        fn notify(&self, _id: usize) {}
    }

    const NOOP : &'static Noop = &Noop;

    NotifyHandle::from(NOOP)
}

#[bench]
fn task_init(b: &mut Bencher) {
    const NUM: u32 = 100_000;

    struct MyFuture {
        num: u32,
        task: Option<Waker>,
    };

    impl Future for MyFuture {
        type Item = ();
        type Error = ();

        fn poll(&mut self, cx: &mut task::Context) -> Poll<(), ()> {
            if self.num == NUM {
                Ok(Async::Ready(()))
            } else {
                self.num += 1;

                if let Some(ref t) = self.task {
                    t.notify();
                    return Ok(Async::Pending);
                }

                let t = cx.waker();
                t.notify();
                self.task = Some(t);

                Ok(Async::Pending)
            }
        }
    }

    let mut fut = MyFuture {
        num: 0,
        task: None,
    };
    let mut notify = || notify_noop().into();
    let mut map = LocalMap::new();
    let mut cx = task::Context::new(&mut map, 0, &mut notify);

    b.iter(|| {
        fut.num = 0;

        while let Ok(Async::Pending) = fut.poll(&mut cx) {
        }
    });
}
