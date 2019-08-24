#![cfg(feature = "bytes")]

use bytes_crate::BytesMut;
use futures::executor::block_on;
use futures::io::{AsyncRead, AsyncReadExt, BufMut, Initializer};
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

#[test]
fn read_buf() {
    let mut buf = BytesMut::with_capacity(65);

    let mut reader = &b"hello world"[..];

    let n = block_on(reader.read_buf(&mut buf)).unwrap();
    assert_eq!(11, n);
    assert_eq!(buf[..], b"hello world"[..]);
}

#[test]
fn read_buf_no_capacity() {
    struct Reader;

    impl AsyncRead for Reader {
        fn poll_read(
            self: Pin<&mut Self>,
            _: &mut Context<'_>,
            _: &mut [u8],
        ) -> Poll<io::Result<usize>> {
            unimplemented!();
        }
    }

    // Can't create BytesMut w/ zero capacity, so fill it up
    let mut buf = BytesMut::with_capacity(64);

    buf.put(&[0; 64][..]);

    let mut reader = Reader;

    let n = block_on(reader.read_buf(&mut buf)).unwrap();
    assert_eq!(0, n);
}

#[test]
fn read_buf_no_uninitialized() {
    struct Reader;

    impl AsyncRead for Reader {
        fn poll_read(
            self: Pin<&mut Self>,
            _: &mut Context<'_>,
            buf: &mut [u8],
        ) -> Poll<io::Result<usize>> {
            for b in buf {
                assert_eq!(0, *b);
            }

            Poll::Ready(Ok(0))
        }
    }

    let mut buf = BytesMut::with_capacity(64);

    let mut reader = Reader;

    let n = block_on(reader.read_buf(&mut buf)).unwrap();
    assert_eq!(0, n);
}

#[test]
fn read_buf_uninitialized_ok() {
    struct Reader;

    impl AsyncRead for Reader {
        unsafe fn initializer(&self) -> Initializer {
            Initializer::nop()
        }

        fn poll_read(
            self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
            buf: &mut [u8],
        ) -> Poll<io::Result<usize>> {
            assert_eq!(buf[0..11], b"hello world"[..]);
            Poll::Ready(Ok(0))
        }
    }

    // Can't create BytesMut w/ zero capacity, so fill it up
    let mut buf = BytesMut::with_capacity(64);

    unsafe {
        buf.bytes_mut()[0..11].copy_from_slice(b"hello world");
    }

    let mut reader = Reader;

    let n = block_on(reader.read_buf(&mut buf)).unwrap();
    assert_eq!(0, n);
}
