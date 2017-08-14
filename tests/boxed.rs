#![cfg(rust_nightly)]

extern crate futures;

use futures::Async;
use futures::Poll;
use futures::future::Future;
use futures::stream::Stream;


#[test]
fn future_boxed_prevents_double_boxing() {
    struct MyFuture {
        r: &'static str,
    }

    impl Future for MyFuture {
        type Item = &'static str;
        type Error = ();

        fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
            Ok(Async::Ready(self.r))
        }
    }

    let f = MyFuture { r: "I'm ready" };
    let f = f.boxed();
    let ptr = f.as_ref() as *const Future<Item=_, Error=_>;
    let f = f.boxed();
    let f = f.boxed();
    let mut f = f.boxed();
    assert_eq!(f.as_ref() as *const Future<Item=_, Error=_>, ptr);
    assert_eq!(Ok(Async::Ready("I'm ready")), f.poll());
}

#[test]
fn stream_boxed_prevents_double_boxing() {
    struct MyStream {
        i: u32,
    }

    impl Stream for MyStream {
        type Item = u32;
        type Error = ();

        fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
            self.i += 1;
            Ok(Async::Ready(Some(self.i)))
        }
    }

    let s = MyStream { i: 0 };
    let s = s.boxed();
    let ptr = s.as_ref() as *const Stream<Item=_, Error=_>;
    let s = s.boxed();
    let s = s.boxed();
    let mut s = s.boxed();
    assert_eq!(s.as_ref() as *const Stream<Item=_, Error=_>, ptr);
    assert_eq!(Ok(Async::Ready(Some(1))), s.poll());
    assert_eq!(Ok(Async::Ready(Some(2))), s.poll());
    assert_eq!(Ok(Async::Ready(Some(3))), s.poll());
}
