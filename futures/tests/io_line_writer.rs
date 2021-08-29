use futures::executor::block_on;
use futures::io::AsyncWrite;
use futures_test::task::panic_context;
use std::io;
use std::{pin::Pin, task::Poll};

use futures::io::{AsyncWriteExt, LineWriter};

#[test]
fn line_writer() {
    let mut writer = LineWriter::new(Vec::new());

    block_on(writer.write(&[0])).unwrap();
    assert_eq!(*writer.get_ref(), []);

    block_on(writer.write(&[1])).unwrap();
    assert_eq!(*writer.get_ref(), []);

    block_on(writer.flush()).unwrap();
    assert_eq!(*writer.get_ref(), [0, 1]);

    block_on(writer.write(&[0, b'\n', 1, b'\n', 2])).unwrap();
    assert_eq!(*writer.get_ref(), [0, 1, 0, b'\n', 1, b'\n']);

    block_on(writer.flush()).unwrap();
    assert_eq!(*writer.get_ref(), [0, 1, 0, b'\n', 1, b'\n', 2]);

    block_on(writer.write(&[3, b'\n'])).unwrap();
    assert_eq!(*writer.get_ref(), [0, 1, 0, b'\n', 1, b'\n', 2, 3, b'\n']);
}

#[test]
fn line_vectored() {
    let mut line_writer = LineWriter::new(Vec::new());
    let cx = &mut panic_context();
    {
        let bufs = &mut [
            io::IoSlice::new(&[]),
            io::IoSlice::new(b"\n"),
            io::IoSlice::new(&[]),
            io::IoSlice::new(b"a"),
        ];
        let result = Pin::new(&mut line_writer).poll_write_vectored(cx, bufs);
        let result = result.map_err(|e| e.kind());
        assert_eq!(result, Poll::Ready(Ok(2)))
    }
    assert_eq!(line_writer.get_ref(), b"\n");

    {
        let bufs = &mut [
            io::IoSlice::new(&[]),
            io::IoSlice::new(b"b"),
            io::IoSlice::new(&[]),
            io::IoSlice::new(b"a"),
            io::IoSlice::new(&[]),
            io::IoSlice::new(b"c"),
        ];
        let result = Pin::new(&mut line_writer).poll_write_vectored(cx, bufs);
        let result = result.map_err(|e| e.kind());
        assert_eq!(result, Poll::Ready(Ok(3)))
    }
    assert_eq!(line_writer.get_ref(), b"\n");
    block_on(line_writer.flush()).unwrap();
    assert_eq!(line_writer.get_ref(), b"\nabac");
    {
        let res = Pin::new(&mut line_writer).poll_write_vectored(cx, &[]);
        let res = res.map_err(|e| e.kind());
        assert_eq!(res, Poll::Ready(Ok(0)));
    }

    {
        let bufs = &mut [
            io::IoSlice::new(&[]),
            io::IoSlice::new(&[]),
            io::IoSlice::new(&[]),
            io::IoSlice::new(&[]),
        ];
        let result = Pin::new(&mut line_writer).poll_write_vectored(cx, bufs);
        let result = result.map_err(|e| e.kind());
        assert_eq!(result, Poll::Ready(Ok(0)))
    }
    {
        let result =
            Pin::new(&mut line_writer).poll_write_vectored(cx, &[io::IoSlice::new(b"a\nb")]);
        let result = result.map_err(|e| e.kind());
        assert_eq!(result, Poll::Ready(Ok(3)))
    }
    assert_eq!(line_writer.get_ref(), b"\nabaca\nb");
}
