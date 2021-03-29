#![feature(test)]

#[cfg(feature = "bilock")]
mod bench {
    use futures::executor::block_on;
    use futures::task::Poll;
    use futures_test::task::noop_context;
    use futures_util::lock::BiLock;

    use std::mem::drop;
    extern crate test;
    use test::Bencher;

    #[bench]
    fn contended(b: &mut Bencher) {
        let mut ctx = noop_context();

        b.iter(|| {
            let (x, y) = BiLock::new(1);

            for _ in 0..1000 {
                let x_guard = match x.poll_lock(&mut ctx) {
                    Poll::Ready(guard) => guard,
                    _ => panic!(),
                };

                // Try poll second lock while first lock still holds the lock
                match y.poll_lock(&mut ctx) {
                    Poll::Pending => (),
                    _ => panic!(),
                };

                drop(x_guard);

                let y_guard = match y.poll_lock(&mut ctx) {
                    Poll::Ready(guard) => guard,
                    _ => panic!(),
                };

                drop(y_guard);
            }
            (x, y)
        });
    }

    #[bench]
    fn lock_unlock(b: &mut Bencher) {
        let mut ctx = noop_context();

        b.iter(|| {
            let (x, y) = BiLock::new(1);

            for _ in 0..1000 {
                let x_guard = match x.poll_lock(&mut ctx) {
                    Poll::Ready(guard) => guard,
                    _ => panic!(),
                };

                drop(x_guard);

                let y_guard = match y.poll_lock(&mut ctx) {
                    Poll::Ready(guard) => guard,
                    _ => panic!(),
                };

                drop(y_guard);
            }
            (x, y)
        })
    }

    #[bench]
    fn concurrent(b: &mut Bencher) {
        use std::thread;

        b.iter(|| {
            let (x, y) = BiLock::new(false);
            const ITERATION_COUNT: usize = 1000;

            let a = thread::spawn(move || {
                let mut count = 0;
                while count < ITERATION_COUNT {
                    let mut guard = block_on(x.lock());
                    if *guard {
                        *guard = false;
                        count += 1;
                    }
                    drop(guard);
                }
            });

            let b = thread::spawn(move || {
                let mut count = 0;
                while count < ITERATION_COUNT {
                    let mut guard = block_on(y.lock());
                    if !*guard {
                        *guard = true;
                        count += 1;
                    }
                    drop(guard);
                }
            });

            a.join().unwrap();
            b.join().unwrap();
        })
    }
}
