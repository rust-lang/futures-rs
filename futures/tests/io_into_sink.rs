use std::io;

use futures::{
    executor::block_on,
    io::{AsyncWriteExt, Cursor},
    sink::SinkExt,
};

#[test]
fn write_zero() {
    let mut buf = [0; 2];
    let mut sink = Cursor::new(&mut buf[..]).into_sink();
    let err = block_on(sink.send(&b"abc"[..])).unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::WriteZero);
    drop(sink);
    assert_eq!(buf, *b"ab");
}

#[test]
fn empty_item() {
    let mut sink = Cursor::new(Vec::new()).into_sink();
    block_on(sink.send(&b""[..])).unwrap();
    block_on(sink.send(&b"a"[..])).unwrap();
}
