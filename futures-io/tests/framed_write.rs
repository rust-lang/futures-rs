extern crate bytes;
extern crate futures;

use futures::prelude::*;
use futures::{executor, io};
use futures::io::codec::{Encoder, FramedWrite};
use futures::Async::{Ready, Pending};

use bytes::{BytesMut, BufMut, BigEndian};

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

struct U32Encoder;

impl Encoder for U32Encoder {
    type Item = u32;
    type Error = io::Error;

    fn encode(&mut self, item: u32, dst: &mut BytesMut) -> io::Result<()> {
        // Reserve space
        dst.reserve(4);
        dst.put_u32::<BigEndian>(item);
        Ok(())
    }
}

#[test]
fn write_multi_frame_in_packet() {
    let mock = mock! {
        Ok(Ready(b"\x00\x00\x00\x00\x00\x00\x00\x01\x00\x00\x00\x02".to_vec())),
    };

    let mut framed = FramedWrite::new(mock, U32Encoder);
    framed.start_send(0).unwrap();
    framed.start_send(1).unwrap();
    framed.start_send(2).unwrap();

    // Nothing written yet
    assert_eq!(1, framed.get_ref().calls.len());

    // Flush the writes
    dummy_cx(|cx| {
        assert!(framed.poll_flush(cx).unwrap().is_ready());
    });

    assert_eq!(0, framed.get_ref().calls.len());
}

#[test]
fn write_hits_backpressure() {
    const ITER: usize = 2 * 1024;

    let mut mock = mock! {
        // Block the `ITER`th write
        Ok(Pending),
        Ok(Ready(b"".to_vec())),
    };

    for i in 0..(ITER + 1) {
        let mut b = BytesMut::with_capacity(4);
        b.put_u32::<BigEndian>(i as u32);

        // Append to the end
        match mock.calls.back_mut().unwrap() {
            &mut Ok(Ready(ref mut data)) => {
                // Write in 2kb chunks
                if data.len() < ITER {
                    data.extend_from_slice(&b[..]);
                    continue;
                }
            }
            _ => unreachable!(),
        }

        // Push a new new chunk
        mock.calls.push_back(Ok(Ready(b[..].to_vec())));
    }

    let mut framed = FramedWrite::new(mock, U32Encoder);

    for i in 0..ITER {
        framed.start_send(i as u32).unwrap();
    }

    dummy_cx(|cx| {
        // This should reject
        assert!(!framed.poll_ready(cx).unwrap().is_ready());

        // This should succeed and start flushing the buffer.
        framed.start_send(ITER as u32).unwrap();

        // Flush the rest of the buffer
        assert!(framed.poll_flush(cx).unwrap().is_ready());
    });

    // Ensure the mock is empty
    assert_eq!(0, framed.get_ref().calls.len());
}

// ===== Mock ======

struct Mock {
    calls: VecDeque<Poll<Vec<u8>, io::Error>>,
}

impl AsyncWrite for Mock {
    fn poll_write(&mut self, _: &mut task::Context, src: &[u8]) -> Poll<usize, io::Error> {
        match self.calls.pop_front().expect("unexpected write") {
            Ok(Ready(data)) => {
                assert!(src.len() >= data.len());
                assert_eq!(&data[..], &src[..data.len()]);
                Ok(Ready(data.len()))
            }
            Ok(Pending) => Ok(Pending),
            Err(e) => Err(e),
        }
    }

    fn poll_flush(&mut self, _: &mut task::Context) -> Poll<(), io::Error> {
        Ok(Ready(()))
    }

    fn poll_close(&mut self, _: &mut task::Context) -> Poll<(), io::Error> {
        Ok(Ready(()))
    }
}
