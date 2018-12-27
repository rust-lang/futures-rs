#![feature(pin, async_await, arbitrary_self_types, futures_api)]

use futures::future::{Future, FutureExt, FutureObj};
use std::pin::Pin;
use futures::task::{LocalWaker, Poll};

#[test]
fn dropping_does_not_segfault() {
    FutureObj::new(async { String::new() }.boxed());
}

#[test]
fn dropping_drops_the_future() {
    let mut times_dropped = 0;

    struct Inc<'a>(&'a mut u32);

    impl<'a> Future for Inc<'a> {
        type Output = ();

        fn poll(self: Pin<&mut Self>, _: &LocalWaker) -> Poll<()> {
            unimplemented!()
        }
    }

    impl<'a> Drop for Inc<'a> {
        fn drop(&mut self) {
            *self.0 += 1;
        }
    }

    FutureObj::new(Inc(&mut times_dropped).boxed());

    assert_eq!(times_dropped, 1);
}
