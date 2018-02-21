extern crate futures;

use futures::prelude::*;
use futures::{executor, io};
use futures::io::codec::length_delimited::*;
use futures::Async::{Ready, Pending};

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

#[test]
fn read_empty_io_yields_nothing() {
    let mut io = FramedRead::new(mock!());

    dummy_cx(|cx| {
        assert_eq!(io.poll_next(cx).unwrap(), Ready(None));
    })
}

#[test]
fn read_single_frame_one_packet() {
    let mut io = FramedRead::new(mock! {
        Ok(Ready(b"\x00\x00\x00\x09abcdefghi"[..].into())),
    });

    dummy_cx(|cx| {
        assert_eq!(io.poll_next(cx).unwrap(), Ready(Some(b"abcdefghi"[..].into())));
        assert_eq!(io.poll_next(cx).unwrap(), Ready(None));
    })
}

#[test]
fn read_single_frame_one_packet_little_endian() {
    let mut io = Builder::new()
        .little_endian()
        .new_read(mock! {
            Ok(Ready(b"\x09\x00\x00\x00abcdefghi"[..].into())),
        });

    dummy_cx(|cx| {
        assert_eq!(io.poll_next(cx).unwrap(), Ready(Some(b"abcdefghi"[..].into())));
        assert_eq!(io.poll_next(cx).unwrap(), Ready(None));
    })
}

#[test]
fn read_single_multi_frame_one_packet() {
    let mut data: Vec<u8> = vec![];
    data.extend_from_slice(b"\x00\x00\x00\x09abcdefghi");
    data.extend_from_slice(b"\x00\x00\x00\x03123");
    data.extend_from_slice(b"\x00\x00\x00\x0bhello world");

    let mut io = FramedRead::new(mock! {
        Ok(Ready(data.into())),
    });

    dummy_cx(|cx| {
        assert_eq!(io.poll_next(cx).unwrap(), Ready(Some(b"abcdefghi"[..].into())));
        assert_eq!(io.poll_next(cx).unwrap(), Ready(Some(b"123"[..].into())));
        assert_eq!(io.poll_next(cx).unwrap(), Ready(Some(b"hello world"[..].into())));
        assert_eq!(io.poll_next(cx).unwrap(), Ready(None));
    })
}

#[test]
fn read_single_frame_multi_packet() {
    let mut io = FramedRead::new(mock! {
        Ok(Ready(b"\x00\x00"[..].into())),
        Ok(Ready(b"\x00\x09abc"[..].into())),
        Ok(Ready(b"defghi"[..].into())),
    });

    dummy_cx(|cx| {
        assert_eq!(io.poll_next(cx).unwrap(), Ready(Some(b"abcdefghi"[..].into())));
        assert_eq!(io.poll_next(cx).unwrap(), Ready(None));
    })
}

#[test]
fn read_multi_frame_multi_packet() {
    let mut io = FramedRead::new(mock! {
        Ok(Ready(b"\x00\x00"[..].into())),
        Ok(Ready(b"\x00\x09abc"[..].into())),
        Ok(Ready(b"defghi"[..].into())),
        Ok(Ready(b"\x00\x00\x00\x0312"[..].into())),
        Ok(Ready(b"3\x00\x00\x00\x0bhello world"[..].into())),
    });

    dummy_cx(|cx| {
        assert_eq!(io.poll_next(cx).unwrap(), Ready(Some(b"abcdefghi"[..].into())));
        assert_eq!(io.poll_next(cx).unwrap(), Ready(Some(b"123"[..].into())));
        assert_eq!(io.poll_next(cx).unwrap(), Ready(Some(b"hello world"[..].into())));
        assert_eq!(io.poll_next(cx).unwrap(), Ready(None));
    })
}

#[test]
fn read_single_frame_multi_packet_wait() {
    let mut io = FramedRead::new(mock! {
        Ok(Ready(b"\x00\x00"[..].into())),
        Ok(Pending),
        Ok(Ready(b"\x00\x09abc"[..].into())),
        Ok(Pending),
        Ok(Ready(b"defghi"[..].into())),
        Ok(Pending),
    });

    dummy_cx(|cx| {
        assert_eq!(io.poll_next(cx).unwrap(), Pending);
        assert_eq!(io.poll_next(cx).unwrap(), Pending);
        assert_eq!(io.poll_next(cx).unwrap(), Ready(Some(b"abcdefghi"[..].into())));
        assert_eq!(io.poll_next(cx).unwrap(), Pending);
        assert_eq!(io.poll_next(cx).unwrap(), Ready(None));
    })
}

#[test]
fn read_multi_frame_multi_packet_wait() {
    let mut io = FramedRead::new(mock! {
        Ok(Ready(b"\x00\x00"[..].into())),
        Ok(Pending),
        Ok(Ready(b"\x00\x09abc"[..].into())),
        Ok(Pending),
        Ok(Ready(b"defghi"[..].into())),
        Ok(Pending),
        Ok(Ready(b"\x00\x00\x00\x0312"[..].into())),
        Ok(Pending),
        Ok(Ready(b"3\x00\x00\x00\x0bhello world"[..].into())),
        Ok(Pending),
    });

    dummy_cx(|cx| {
        assert_eq!(io.poll_next(cx).unwrap(), Pending);
        assert_eq!(io.poll_next(cx).unwrap(), Pending);
        assert_eq!(io.poll_next(cx).unwrap(), Ready(Some(b"abcdefghi"[..].into())));
        assert_eq!(io.poll_next(cx).unwrap(), Pending);
        assert_eq!(io.poll_next(cx).unwrap(), Pending);
        assert_eq!(io.poll_next(cx).unwrap(), Ready(Some(b"123"[..].into())));
        assert_eq!(io.poll_next(cx).unwrap(), Ready(Some(b"hello world"[..].into())));
        assert_eq!(io.poll_next(cx).unwrap(), Pending);
        assert_eq!(io.poll_next(cx).unwrap(), Ready(None));
    })
}

#[test]
fn read_incomplete_head() {
    let mut io = FramedRead::new(mock! {
        Ok(Ready(b"\x00\x00"[..].into())),
    });

    dummy_cx(|cx| {
        assert!(io.poll_next(cx).is_err());
    })
}

#[test]
fn read_incomplete_head_multi() {
    let mut io = FramedRead::new(mock! {
        Ok(Pending),
        Ok(Ready(b"\x00"[..].into())),
        Ok(Pending),
    });

    dummy_cx(|cx| {
        assert_eq!(io.poll_next(cx).unwrap(), Pending);
        assert_eq!(io.poll_next(cx).unwrap(), Pending);
        assert!(io.poll_next(cx).is_err());
    })
}

#[test]
fn read_incomplete_payload() {
    let mut io = FramedRead::new(mock! {
        Ok(Ready(b"\x00\x00\x00\x09ab"[..].into())),
        Ok(Pending),
        Ok(Ready(b"cd"[..].into())),
        Ok(Pending),
    });

    dummy_cx(|cx| {
        assert_eq!(io.poll_next(cx).unwrap(), Pending);
        assert_eq!(io.poll_next(cx).unwrap(), Pending);
        assert!(io.poll_next(cx).is_err());
    })
}

#[test]
fn read_max_frame_len() {
    let mut io = Builder::new()
        .max_frame_length(5)
        .new_read(mock! {
            Ok(Ready(b"\x00\x00\x00\x09abcdefghi"[..].into())),
        });

    dummy_cx(|cx| {
        assert_eq!(io.poll_next(cx).unwrap_err().kind(), io::ErrorKind::InvalidData);
    })
}

#[test]
fn read_update_max_frame_len_at_rest() {
    let mut io = Builder::new()
        .new_read(mock! {
            Ok(Ready(b"\x00\x00\x00\x09abcdefghi"[..].into())),
            Ok(Ready(b"\x00\x00\x00\x09abcdefghi"[..].into())),
        });

    dummy_cx(|cx| {
        assert_eq!(io.poll_next(cx).unwrap(), Ready(Some(b"abcdefghi"[..].into())));
        io.set_max_frame_length(5);
        assert_eq!(io.poll_next(cx).unwrap_err().kind(), io::ErrorKind::InvalidData);
    })
}

#[test]
fn read_update_max_frame_len_in_flight() {
    let mut io = Builder::new()
        .new_read(mock! {
            Ok(Ready(b"\x00\x00\x00\x09abcd"[..].into())),
            Ok(Pending),
            Ok(Ready(b"efghi"[..].into())),
            Ok(Ready(b"\x00\x00\x00\x09abcdefghi"[..].into())),
        });

    dummy_cx(|cx| {
        assert_eq!(io.poll_next(cx).unwrap(), Pending);
        io.set_max_frame_length(5);
        assert_eq!(io.poll_next(cx).unwrap(), Ready(Some(b"abcdefghi"[..].into())));
        assert_eq!(io.poll_next(cx).unwrap_err().kind(), io::ErrorKind::InvalidData);
    })
}

#[test]
fn read_one_byte_length_field() {
    let mut io = Builder::new()
        .length_field_length(1)
        .new_read(mock! {
            Ok(Ready(b"\x09abcdefghi"[..].into())),
        });

    dummy_cx(|cx| {
        assert_eq!(io.poll_next(cx).unwrap(), Ready(Some(b"abcdefghi"[..].into())));
        assert_eq!(io.poll_next(cx).unwrap(), Ready(None));
    })
}

#[test]
fn read_header_offset() {
    let mut io = Builder::new()
        .length_field_length(2)
        .length_field_offset(4)
        .new_read(mock! {
            Ok(Ready(b"zzzz\x00\x09abcdefghi"[..].into())),
        });

    dummy_cx(|cx| {
        assert_eq!(io.poll_next(cx).unwrap(), Ready(Some(b"abcdefghi"[..].into())));
        assert_eq!(io.poll_next(cx).unwrap(), Ready(None));
    })
}

#[test]
fn read_single_multi_frame_one_packet_skip_none_adjusted() {
    let mut data: Vec<u8> = vec![];
    data.extend_from_slice(b"xx\x00\x09abcdefghi");
    data.extend_from_slice(b"yy\x00\x03123");
    data.extend_from_slice(b"zz\x00\x0bhello world");

    let mut io = Builder::new()
        .length_field_length(2)
        .length_field_offset(2)
        .num_skip(0)
        .length_adjustment(4)
        .new_read(mock! {
            Ok(Ready(data.into())),
        });

    dummy_cx(|cx| {
        assert_eq!(io.poll_next(cx).unwrap(), Ready(Some(b"xx\x00\x09abcdefghi"[..].into())));
        assert_eq!(io.poll_next(cx).unwrap(), Ready(Some(b"yy\x00\x03123"[..].into())));
        assert_eq!(io.poll_next(cx).unwrap(), Ready(Some(b"zz\x00\x0bhello world"[..].into())));
        assert_eq!(io.poll_next(cx).unwrap(), Ready(None));
    })
}

#[test]
fn read_single_multi_frame_one_packet_length_includes_head() {
    let mut data: Vec<u8> = vec![];
    data.extend_from_slice(b"\x00\x0babcdefghi");
    data.extend_from_slice(b"\x00\x05123");
    data.extend_from_slice(b"\x00\x0dhello world");

    let mut io = Builder::new()
        .length_field_length(2)
        .length_adjustment(-2)
        .new_read(mock! {
            Ok(Ready(data.into())),
        });

    dummy_cx(|cx| {
        assert_eq!(io.poll_next(cx).unwrap(), Ready(Some(b"abcdefghi"[..].into())));
        assert_eq!(io.poll_next(cx).unwrap(), Ready(Some(b"123"[..].into())));
        assert_eq!(io.poll_next(cx).unwrap(), Ready(Some(b"hello world"[..].into())));
        assert_eq!(io.poll_next(cx).unwrap(), Ready(None));
    })
}

#[test]
fn write_single_frame_length_adjusted() {
    let mut io = Builder::new()
        .length_adjustment(-2)
        .new_write(mock! {
            Ok(Ready(b"\x00\x00\x00\x0b"[..].into())),
            Ok(Ready(b"abcdefghi"[..].into())),
            Ok(Ready(Flush)),
        });

    dummy_cx(|cx| {
        io.start_send("abcdefghi").unwrap();
        assert!(io.poll_ready(cx).unwrap().is_ready());
        assert!(io.get_ref().calls.is_empty());
    })
}

#[test]
fn write_nothing_yields_nothing() {
    let mut io: FramedWrite<_, &'static [u8]> = FramedWrite::new(mock!());
    dummy_cx(|cx| {
        assert!(io.poll_flush(cx).unwrap().is_ready());
    })
}

#[test]
fn write_single_frame_one_packet() {
    let mut io = FramedWrite::new(mock! {
        Ok(Ready(b"\x00\x00\x00\x09"[..].into())),
        Ok(Ready(b"abcdefghi"[..].into())),
        Ok(Ready(Flush)),
    });

    dummy_cx(|cx| {
        io.start_send("abcdefghi").unwrap();
        assert!(io.poll_flush(cx).unwrap().is_ready());
        assert!(io.get_ref().calls.is_empty());
    })
}

#[test]
fn write_single_multi_frame_one_packet() {
    let mut io = FramedWrite::new(mock! {
        Ok(Ready(b"\x00\x00\x00\x09"[..].into())),
        Ok(Ready(b"abcdefghi"[..].into())),
        Ok(Ready(b"\x00\x00\x00\x03"[..].into())),
        Ok(Ready(b"123"[..].into())),
        Ok(Ready(b"\x00\x00\x00\x0b"[..].into())),
        Ok(Ready(b"hello world"[..].into())),
        Ok(Ready(Flush)),
    });
    dummy_cx(|cx| {
        assert!(io.poll_ready(cx).unwrap().is_ready());
        io.start_send("abcdefghi").unwrap();
        assert!(io.poll_ready(cx).unwrap().is_ready());
        io.start_send("123").unwrap();
        assert!(io.poll_ready(cx).unwrap().is_ready());
        io.start_send("hello world").unwrap();
        assert!(io.poll_flush(cx).unwrap().is_ready());
    });
    assert!(io.get_ref().calls.is_empty());
}

#[test]
fn write_single_multi_frame_multi_packet() {
    let mut io = FramedWrite::new(mock! {
        Ok(Ready(b"\x00\x00\x00\x09"[..].into())),
        Ok(Ready(b"abcdefghi"[..].into())),
        Ok(Ready(Flush)),
        Ok(Ready(b"\x00\x00\x00\x03"[..].into())),
        Ok(Ready(b"123"[..].into())),
        Ok(Ready(Flush)),
        Ok(Ready(b"\x00\x00\x00\x0b"[..].into())),
        Ok(Ready(b"hello world"[..].into())),
        Ok(Ready(Flush)),
    });

    dummy_cx(|cx| {
        io.start_send("abcdefghi").unwrap();
        assert!(io.poll_flush(cx).unwrap().is_ready());
        io.start_send("123").unwrap();
        assert!(io.poll_flush(cx).unwrap().is_ready());
        io.start_send("hello world").unwrap();
        assert!(io.poll_flush(cx).unwrap().is_ready());
        assert!(io.get_ref().calls.is_empty());
    })
}

#[test]
fn write_single_frame_would_block() {
    let mut io = FramedWrite::new(mock! {
        Ok(Pending),
        Ok(Ready(b"\x00\x00"[..].into())),
        Ok(Pending),
        Ok(Ready(b"\x00\x09"[..].into())),
        Ok(Ready(b"abcdefghi"[..].into())),
        Ok(Ready(Flush)),
    });

    dummy_cx(|cx| {
        io.start_send("abcdefghi").unwrap();
        assert!(!io.poll_flush(cx).unwrap().is_ready());
        assert!(!io.poll_flush(cx).unwrap().is_ready());
        assert!(io.poll_flush(cx).unwrap().is_ready());

        assert!(io.get_ref().calls.is_empty());
    })
}

#[test]
fn write_single_frame_little_endian() {
    let mut io = Builder::new()
        .little_endian()
        .new_write(mock! {
            Ok(Ready(b"\x09\x00\x00\x00"[..].into())),
            Ok(Ready(b"abcdefghi"[..].into())),
            Ok(Ready(Flush)),
        });

    dummy_cx(|cx| {
        io.start_send("abcdefghi").unwrap();
        assert!(io.poll_flush(cx).unwrap().is_ready());
        assert!(io.get_ref().calls.is_empty());
    })
}


#[test]
fn write_single_frame_with_short_length_field() {
    let mut io = Builder::new()
        .length_field_length(1)
        .new_write(mock! {
            Ok(Ready(b"\x09"[..].into())),
            Ok(Ready(b"abcdefghi"[..].into())),
            Ok(Ready(Flush)),
        });

    dummy_cx(|cx| {
        io.start_send("abcdefghi").unwrap();
        assert!(io.poll_flush(cx).unwrap().is_ready());
        assert!(io.get_ref().calls.is_empty());
    })
}

#[test]
fn write_max_frame_len() {
    let mut io = Builder::new()
        .max_frame_length(5)
        .new_write(mock! { });

    assert_eq!(io.start_send("abcdef").unwrap_err().kind(), io::ErrorKind::InvalidInput);
    assert!(io.get_ref().calls.is_empty());
}

#[test]
fn write_update_max_frame_len_at_rest() {
    let mut io = Builder::new()
        .new_write(mock! {
            Ok(Ready(b"\x00\x00\x00\x06"[..].into())),
            Ok(Ready(b"abcdef"[..].into())),
            Ok(Ready(Flush)),
        });

    io.start_send("abcdef").unwrap();
    dummy_cx(|cx| {
        assert!(io.poll_flush(cx).unwrap().is_ready());
    });
    io.set_max_frame_length(5);
    assert_eq!(io.start_send("abcdef").unwrap_err().kind(), io::ErrorKind::InvalidInput);
    assert!(io.get_ref().calls.is_empty());
}

#[test]
fn write_update_max_frame_len_in_flight() {
    let mut io = Builder::new()
        .new_write(mock! {
            Ok(Ready(b"\x00\x00\x00\x06"[..].into())),
            Ok(Ready(b"ab"[..].into())),
            Ok(Pending),
            Ok(Ready(b"cdef"[..].into())),
            Ok(Ready(Flush)),
        });

    dummy_cx(|cx| {
        io.start_send("abcdef").unwrap();
        assert!(!io.poll_flush(cx).unwrap().is_ready());
        io.set_max_frame_length(5);
        assert!(io.poll_flush(cx).unwrap().is_ready());
        assert_eq!(io.start_send("abcdef").unwrap_err().kind(), io::ErrorKind::InvalidInput);
        assert!(io.poll_flush(cx).unwrap().is_ready());
        assert!(io.get_ref().calls.is_empty());
    })
}

// ===== Test utils =====

struct Mock {
    calls: VecDeque<Poll<Op, io::Error>>,
}

#[derive(Debug, PartialEq)]
enum Op {
    Data(Vec<u8>),
    Flush,
}

use self::Op::*;

impl AsyncRead for Mock {
    fn poll_read(&mut self, _: &mut task::Context, dst: &mut [u8]) -> Poll<usize, io::Error> {
        match self.calls.pop_front() {
            Some(Ok(Ready(Op::Data(data)))) => {
                debug_assert!(dst.len() >= data.len());
                dst[..data.len()].copy_from_slice(&data[..]);
                Ok(Ready(data.len()))
            }
            Some(Ok(Ready(_))) => panic!(),
            Some(Ok(Pending)) => Ok(Pending),
            Some(Err(e)) => Err(e),
            None => Ok(Ready(0)),
        }
    }
}

impl AsyncWrite for Mock {
    fn poll_write(&mut self, _: &mut task::Context, src: &[u8]) -> Poll<usize, io::Error> {
        match self.calls.pop_front() {
            Some(Ok(Ready(Op::Data(data)))) => {
                let len = data.len();
                assert!(src.len() >= len, "expect={:?}; actual={:?}", data, src);
                assert_eq!(&data[..], &src[..len]);
                Ok(Ready(len))
            }
            Some(Ok(Ready(Op::Flush))) => panic!("unexpected flush"),
            Some(Ok(Pending)) => Ok(Pending),
            Some(Err(e)) => Err(e),
            None => Ok(Ready(0)),
        }
    }

    fn poll_flush(&mut self, _: &mut task::Context) -> Poll<(), io::Error> {
        match self.calls.pop_front() {
            Some(Ok(Ready(Op::Flush))) => {
                Ok(Ready(()))
            }
            Some(Ok(Ready(Op::Data(data)))) => {
                // Push the data back on-- it wasn't intended to be received yet
                self.calls.push_front(Ok(Ready(Op::Data(data))));
                Ok(Ready(()))
            },
            Some(Ok(Pending)) => Ok(Pending),
            Some(Err(e)) => Err(e),
            None => Ok(Ready(())),
        }
    }

    fn poll_close(&mut self, _: &mut task::Context) -> Poll<(), io::Error> {
        Ok(Ready(()))
    }
}

impl<'a> From<&'a [u8]> for Op {
    fn from(src: &'a [u8]) -> Op {
        Op::Data(src.into())
    }
}

impl From<Vec<u8>> for Op {
    fn from(src: Vec<u8>) -> Op {
        Op::Data(src)
    }
}
