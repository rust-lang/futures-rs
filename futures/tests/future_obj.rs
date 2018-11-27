#![feature(async_await, futures_api)]

use futures::future::{Future, FutureExt, FutureObj};
use std::pin::Pin;
use futures::task::{Context, Poll};

#[test]
fn dropping_does_not_segfault() {
    FutureObj::new(Box::new(async { String::new() }));
}

#[test]
fn dropping_drops_the_future() {
    let mut times_dropped = 0;

    struct Inc<'a>(&'a mut u32);

    impl Future for Inc<'_> {
        type Output = ();

        fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<()> {
            unimplemented!()
        }
    }

    impl Drop for Inc<'_> {
        fn drop(&mut self) {
            *self.0 += 1;
        }
    }

    FutureObj::new(Box::new(Inc(&mut times_dropped)));

    assert_eq!(times_dropped, 1);
}
