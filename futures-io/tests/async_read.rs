extern crate bytes;
extern crate futures;

use futures::prelude::*;
use futures::{executor, io};
use bytes::{BytesMut, BufMut};

const TEST_MSG: &[u8] = b"hello world";

fn dummy_cx<F>(f: F) where F: FnOnce(&mut task::Context) {
    struct Fut<F>(Option<F>);

    impl<F> Future for Fut<F> where F: FnOnce(&mut task::Context) {
        type Item = ();
        type Error = ();
        fn poll(&mut self, cx: &mut task::Context) -> Poll<(), ()> {
            (self.0.take().unwrap())(cx);
            Ok(Async::Ready(()))
        }
    }

    executor::block_on(Fut(Some(f))).unwrap();
}

#[test]
fn poll_read_buf_success() {
    struct R;

    impl AsyncRead for R {
        fn poll_read(&mut self, _: &mut task::Context, buf: &mut [u8]) -> Poll<usize, io::Error> {
            let len = TEST_MSG.len();
            buf[0..len].copy_from_slice(TEST_MSG);
            Ok(Async::Ready(len))
        }
    }

    let mut buf = BytesMut::with_capacity(65);

    dummy_cx(|cx| {
        let n = match R.poll_read_buf(cx, &mut buf).unwrap() {
            Async::Ready(n) => n,
            _ => panic!(),
        };

        assert_eq!(TEST_MSG.len(), n);
        assert_eq!(buf, TEST_MSG);
    })
}

#[test]
fn poll_read_buf_no_capacity() {
    struct R;

    impl AsyncRead for R {
        fn poll_read(&mut self, _: &mut task::Context, _: &mut [u8]) -> Poll<usize, io::Error> {
            unimplemented!()
        }
    }

    // Can't create BytesMut w/ zero capacity, so fill it up
    let mut buf = BytesMut::with_capacity(64);
    buf.put(&[0; 64][..]);

    dummy_cx(|cx| {
        let n = match R.poll_read_buf(cx, &mut buf).unwrap() {
            Async::Ready(n) => n,
            _ => panic!(),
        };

        assert_eq!(0, n);
    })
}

#[test]
fn poll_read_buf_no_uninitialized() {
    struct R;

    impl AsyncRead for R {
        fn poll_read(&mut self, _: &mut task::Context, buf: &mut [u8]) -> Poll<usize, io::Error> {
            for b in buf {
                assert_eq!(0, *b);
            }

            Ok(Async::Ready(0))
        }
    }

    // Can't create BytesMut w/ zero capacity, so fill it up
    let mut buf = BytesMut::with_capacity(64);

    dummy_cx(|cx| {
        let n = match R.poll_read_buf(cx, &mut buf).unwrap() {
            Async::Ready(n) => n,
            _ => panic!(),
        };

        assert_eq!(0, n);
    })
}

#[test]
fn poll_read_buf_uninitialized_ok() {
    struct R;

    impl AsyncRead for R {
        unsafe fn initializer(&self) -> io::Initializer {
            io::Initializer::nop()
        }

        fn poll_read(&mut self, _: &mut task::Context, buf: &mut [u8]) -> Poll<usize, io::Error> {
            assert_eq!(&buf[0..TEST_MSG.len()], TEST_MSG);
            Ok(Async::Ready(0))
        }
    }

    // Can't create BytesMut w/ zero capacity, so fill it up
    let mut buf = BytesMut::with_capacity(64);
    unsafe {
        buf.bytes_mut()[0..TEST_MSG.len()].copy_from_slice(TEST_MSG);
    }

    dummy_cx(|cx| {
        let n = match R.poll_read_buf(cx, &mut buf).unwrap() {
            Async::Ready(n) => n,
            _ => panic!(),
        };

        assert_eq!(0, n);
    })
}
