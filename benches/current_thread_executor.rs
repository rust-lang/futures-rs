#![feature(test)]

extern crate test;
extern crate futures;

use futures::{task, Async};
use futures::executor::CurrentThread;
use futures::future::{lazy, poll_fn};

use test::Bencher;

use std::cell::Cell;
use std::rc::Rc;

#[bench]
fn execute_oneshot(b: &mut Bencher) {
    const ITER: usize = 1000;

    b.iter(move || {
        let cnt = Rc::new(Cell::new(0));

        CurrentThread::run(|_| {
            for _ in 0..ITER {
                let cnt = cnt.clone();
                CurrentThread::execute(lazy(move || {
                    cnt.set(1 + cnt.get());
                    Ok::<(), ()>(())
                }));
            }
        });

        assert_eq!(cnt.get(), ITER);
    });
}

#[bench]
fn execute_yield_many(b: &mut Bencher) {
    const YIELDS: usize = 500;
    const TASKS: usize = 20;

    b.iter(move || {
        let cnt = Rc::new(Cell::new(0));

        CurrentThread::run(|_| {
            for _ in 0..TASKS {
                let cnt = cnt.clone();
                let mut rem = YIELDS;

                CurrentThread::execute(poll_fn(move || {
                    cnt.set(1 + cnt.get());
                    rem -= 1;

                    if rem == 0 {
                        Ok::<_, ()>(().into())
                    } else {
                        task::current().notify();
                        Ok(Async::NotReady)
                    }
                }));
            }
        });

        assert_eq!(cnt.get(), YIELDS * TASKS);
    });
}

#[bench]
fn execute_daisy(b: &mut Bencher) {
    const DEPTH: usize = 1000;

    let cnt = Rc::new(Cell::new(0));

    fn daisy(rem: usize, cnt: Rc<Cell<usize>>) {
        if rem > 0 {
            CurrentThread::execute(lazy(move || {
                cnt.set(1 + cnt.get());
                daisy(rem - 1, cnt);
                Ok(())
            }));
        }
    }

    b.iter(move || {
        cnt.set(0);

        CurrentThread::run(|_| {
            daisy(DEPTH, cnt.clone());
        });

        assert_eq!(cnt.get(), DEPTH);
    });
}
