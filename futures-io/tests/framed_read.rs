extern crate bytes;
extern crate futures;

use futures::prelude::*;
use futures::{executor, io};
use futures::io::codec::{FramedRead, Decoder};
use futures::Async::{Ready, Pending};

use bytes::{BytesMut, Buf, IntoBuf, BigEndian};

use std::collections::VecDeque;

macro_rules! mock {
    ($($x:expr,)*) => {{
        let mut v = VecDeque::new();
        v.extend(vec![$($x),*]);
        Mock { calls: v }
    }};
}

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

struct U32Decoder;

impl Decoder for U32Decoder {
    type Item = u32;
    type Error = io::Error;

    fn decode(&mut self, buf: &mut BytesMut) -> io::Result<Option<u32>> {
        if buf.len() < 4 {
            return Ok(None);
        }

        let n = buf.split_to(4).into_buf().get_u32::<BigEndian>();
        Ok(Some(n))
    }
}

#[test]
fn read_multi_frame_in_packet() {
    let mock = mock! {
        Ok(Ready(b"\x00\x00\x00\x00\x00\x00\x00\x01\x00\x00\x00\x02".to_vec())),
    };

    let mut framed = FramedRead::new(mock, U32Decoder);
    dummy_cx(|cx| {
        assert_eq!(Ready(Some(0)), framed.poll_next(cx).unwrap());
        assert_eq!(Ready(Some(1)), framed.poll_next(cx).unwrap());
        assert_eq!(Ready(Some(2)), framed.poll_next(cx).unwrap());
        assert_eq!(Ready(None), framed.poll_next(cx).unwrap());
    })
}

#[test]
fn read_multi_frame_across_packets() {
    let mock = mock! {
        Ok(Ready(b"\x00\x00\x00\x00".to_vec())),
        Ok(Ready(b"\x00\x00\x00\x01".to_vec())),
        Ok(Ready(b"\x00\x00\x00\x02".to_vec())),
    };

    let mut framed = FramedRead::new(mock, U32Decoder);
    dummy_cx(|cx| {
        assert_eq!(Ready(Some(0)), framed.poll_next(cx).unwrap());
        assert_eq!(Ready(Some(1)), framed.poll_next(cx).unwrap());
        assert_eq!(Ready(Some(2)), framed.poll_next(cx).unwrap());
        assert_eq!(Ready(None), framed.poll_next(cx).unwrap());
    })
}

#[test]
fn read_not_ready() {
    let mock = mock! {
        Ok(Pending),
        Ok(Ready(b"\x00\x00\x00\x00".to_vec())),
        Ok(Ready(b"\x00\x00\x00\x01".to_vec())),
    };

    let mut framed = FramedRead::new(mock, U32Decoder);
    dummy_cx(|cx| {
        assert_eq!(Pending, framed.poll_next(cx).unwrap());
        assert_eq!(Ready(Some(0)), framed.poll_next(cx).unwrap());
        assert_eq!(Ready(Some(1)), framed.poll_next(cx).unwrap());
        assert_eq!(Ready(None), framed.poll_next(cx).unwrap());
    })
}

#[test]
fn read_partial_then_not_ready() {
    let mock = mock! {
        Ok(Ready(b"\x00\x00".to_vec())),
        Ok(Pending),
        Ok(Ready(b"\x00\x00\x00\x00\x00\x01\x00\x00\x00\x02".to_vec())),
    };

    let mut framed = FramedRead::new(mock, U32Decoder);
    dummy_cx(|cx| {
        assert_eq!(Pending, framed.poll_next(cx).unwrap());
        assert_eq!(Ready(Some(0)), framed.poll_next(cx).unwrap());
        assert_eq!(Ready(Some(1)), framed.poll_next(cx).unwrap());
        assert_eq!(Ready(Some(2)), framed.poll_next(cx).unwrap());
        assert_eq!(Ready(None), framed.poll_next(cx).unwrap());
    })
}

#[test]
fn read_err() {
    let mock = mock! {
        Err(io::Error::new(io::ErrorKind::Other, "")),
    };

    let mut framed = FramedRead::new(mock, U32Decoder);
    dummy_cx(|cx| {
        assert_eq!(io::ErrorKind::Other, framed.poll_next(cx).unwrap_err().kind());
    })
}

#[test]
fn read_partial_then_err() {
    let mock = mock! {
        Ok(Ready(b"\x00\x00".to_vec())),
        Err(io::Error::new(io::ErrorKind::Other, "")),
    };

    let mut framed = FramedRead::new(mock, U32Decoder);
    dummy_cx(|cx| {
        assert_eq!(io::ErrorKind::Other, framed.poll_next(cx).unwrap_err().kind());
    })
}

#[test]
fn read_partial_would_block_then_err() {
    let mock = mock! {
        Ok(Ready(b"\x00\x00".to_vec())),
        Ok(Pending),
        Err(io::Error::new(io::ErrorKind::Other, "")),
    };

    let mut framed = FramedRead::new(mock, U32Decoder);
    dummy_cx(|cx| {
        assert_eq!(Pending, framed.poll_next(cx).unwrap());
        assert_eq!(io::ErrorKind::Other, framed.poll_next(cx).unwrap_err().kind());
    })
}

#[test]
fn huge_size() {
    let data = [0; 32 * 1024];

    let mut framed = FramedRead::new(&data[..], BigDecoder);
    dummy_cx(|cx| {
        assert_eq!(Ready(Some(0)), framed.poll_next(cx).unwrap());
        assert_eq!(Ready(None), framed.poll_next(cx).unwrap());
    });

    struct BigDecoder;

    impl Decoder for BigDecoder {
        type Item = u32;
        type Error = io::Error;

        fn decode(&mut self, buf: &mut BytesMut) -> io::Result<Option<u32>> {
            if buf.len() < 32 * 1024 {
                return Ok(None);
            }
            buf.split_to(32 * 1024);
            Ok(Some(0))
        }
    }
}

#[test]
fn data_remaining_is_error() {
    let data = [0; 5];

    let mut framed = FramedRead::new(&data[..], U32Decoder);
    dummy_cx(|cx| {
        assert_eq!(Ready(Some(0)), framed.poll_next(cx).unwrap());
        assert!(framed.poll_next(cx).is_err());
    })
}

#[test]
fn multi_frames_on_eof() {
    struct MyDecoder(Vec<u32>);

    impl Decoder for MyDecoder {
        type Item = u32;
        type Error = io::Error;

        fn decode(&mut self, _buf: &mut BytesMut) -> io::Result<Option<u32>> {
            unreachable!();
        }

        fn decode_eof(&mut self, _buf: &mut BytesMut) -> io::Result<Option<u32>> {
            if self.0.is_empty() {
                return Ok(None);
            }

            Ok(Some(self.0.remove(0)))
        }
    }

    let mut framed = FramedRead::new(mock!(), MyDecoder(vec![0, 1, 2, 3]));
    dummy_cx(|cx| {
        assert_eq!(Ready(Some(0)), framed.poll_next(cx).unwrap());
        assert_eq!(Ready(Some(1)), framed.poll_next(cx).unwrap());
        assert_eq!(Ready(Some(2)), framed.poll_next(cx).unwrap());
        assert_eq!(Ready(Some(3)), framed.poll_next(cx).unwrap());
        assert_eq!(Ready(None), framed.poll_next(cx).unwrap());
    })
}

// ===== Mock ======

struct Mock {
    calls: VecDeque<Poll<Vec<u8>, io::Error>>,
}

impl AsyncRead for Mock {
    fn poll_read(&mut self, _: &mut task::Context, dst: &mut [u8]) -> Poll<usize, io::Error> {
        match self.calls.pop_front() {
            Some(Ok(Ready(data))) => {
                debug_assert!(dst.len() >= data.len());
                dst[..data.len()].copy_from_slice(&data[..]);
                Ok(Ready(data.len()))
            }
            Some(Ok(Pending)) => Ok(Pending),
            Some(Err(e)) => Err(e),
            None => Ok(Ready(0)),
        }
    }
}
