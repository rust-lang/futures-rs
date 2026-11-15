use std::{
    io,
    pin::Pin,
    task::{Context, Poll},
};

use futures::{
    executor::block_on,
    io::{AsyncWrite, AsyncWriteExt, LineWriter},
};

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
    assert_eq!(
        block_on(line_writer.write_vectored(&[
            io::IoSlice::new(&[]),
            io::IoSlice::new(b"\n"),
            io::IoSlice::new(&[]),
            io::IoSlice::new(b"a"),
        ]))
        .unwrap(),
        2
    );
    assert_eq!(line_writer.get_ref(), b"\n");

    assert_eq!(
        block_on(line_writer.write_vectored(&[
            io::IoSlice::new(&[]),
            io::IoSlice::new(b"b"),
            io::IoSlice::new(&[]),
            io::IoSlice::new(b"a"),
            io::IoSlice::new(&[]),
            io::IoSlice::new(b"c"),
        ]))
        .unwrap(),
        3
    );
    assert_eq!(line_writer.get_ref(), b"\n");
    block_on(line_writer.flush()).unwrap();
    assert_eq!(line_writer.get_ref(), b"\nabac");
    assert_eq!(block_on(line_writer.write_vectored(&[])).unwrap(), 0);

    assert_eq!(
        block_on(line_writer.write_vectored(&[
            io::IoSlice::new(&[]),
            io::IoSlice::new(&[]),
            io::IoSlice::new(&[]),
            io::IoSlice::new(&[]),
        ]))
        .unwrap(),
        0
    );

    assert_eq!(block_on(line_writer.write_vectored(&[io::IoSlice::new(b"a\nb")])).unwrap(), 3);
    assert_eq!(line_writer.get_ref(), b"\nabaca\nb");
}

struct CountFlush(usize);

impl AsyncWrite for CountFlush {
    fn poll_write(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Poll::Ready(Ok(buf.len()))
    }

    fn poll_flush(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.0 += 1;
        Poll::Ready(Ok(()))
    }

    fn poll_close(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

#[test]
fn write_does_not_flush_inner() {
    let mut writer = LineWriter::new(CountFlush(0));

    block_on(writer.write(b"a\nb")).unwrap();
    block_on(writer.write_vectored(&[io::IoSlice::new(b"c\n")])).unwrap();
    assert_eq!(writer.get_ref().0, 0);

    block_on(writer.flush()).unwrap();
    assert_eq!(writer.get_ref().0, 1);
}
