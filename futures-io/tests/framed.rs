extern crate bytes;
extern crate futures;

use futures::prelude::*;
use futures::{executor, io};
use futures::io::codec::{Framed, FramedParts, Decoder, Encoder};
use bytes::{BytesMut, Buf, BufMut, IntoBuf, BigEndian};

const INITIAL_CAPACITY: usize = 8 * 1024;

struct U32Codec;

impl Decoder for U32Codec {
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

impl Encoder for U32Codec {
    type Item = u32;
    type Error = io::Error;

    fn encode(&mut self, item: u32, dst: &mut BytesMut) -> io::Result<()> {
        // Reserve space
        dst.reserve(4);
        dst.put_u32::<BigEndian>(item);
        Ok(())
    }
}

struct DontReadIntoThis;

impl AsyncRead for DontReadIntoThis {
    fn poll_read(&mut self, _: &mut task::Context, _: &mut [u8]) -> Poll<usize, io::Error> {
        Err(io::Error::new(io::ErrorKind::Other,
                           "Read into something you weren't supposed to."))
    }
}

#[test]
fn can_read_from_existing_buf() {
    let parts = FramedParts {
        inner: DontReadIntoThis,
        readbuf: vec![0, 0, 0, 42].into(),
        writebuf: BytesMut::with_capacity(0),
    };
    let framed = Framed::from_parts(parts, U32Codec);

    let num = executor::block_on(framed
        .into_future()
        .map(|(first_num, _)| {
            first_num.unwrap()
        }))
        .map_err(|e| e.0)
        .unwrap();
    assert_eq!(num, 42);
}

#[test]
fn external_buf_grows_to_init() {
    let parts = FramedParts {
        inner: DontReadIntoThis,
        readbuf: vec![0, 0, 0, 42].into(),
        writebuf: BytesMut::with_capacity(0),
    };
    let framed = Framed::from_parts(parts, U32Codec);
    let FramedParts { readbuf, .. } = framed.into_parts();

    assert_eq!(readbuf.capacity(), INITIAL_CAPACITY);
}

#[test]
fn external_buf_does_not_shrink() {
    let parts = FramedParts {
        inner: DontReadIntoThis,
        readbuf: vec![0; INITIAL_CAPACITY * 2].into(),
        writebuf: BytesMut::with_capacity(0),
    };
    let framed = Framed::from_parts(parts, U32Codec);
    let FramedParts { readbuf, .. } = framed.into_parts();

    assert_eq!(readbuf.capacity(), INITIAL_CAPACITY * 2);
}
