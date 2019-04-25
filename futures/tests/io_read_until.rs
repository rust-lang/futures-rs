use futures::executor::block_on;
use futures::io::AsyncBufReadExt;
use std::io::Cursor;

#[test]
fn read_until() {
    let mut buf = Cursor::new(&b"12"[..]);
    let mut v = Vec::new();
    assert_eq!(block_on(buf.read_until(b'3', &mut v)).unwrap(), 2);
    assert_eq!(v, b"12");

    let mut buf = Cursor::new(&b"1233"[..]);
    let mut v = Vec::new();
    assert_eq!(block_on(buf.read_until(b'3', &mut v)).unwrap(), 3);
    assert_eq!(v, b"123");
    v.truncate(0);
    assert_eq!(block_on(buf.read_until(b'3', &mut v)).unwrap(), 1);
    assert_eq!(v, b"3");
    v.truncate(0);
    assert_eq!(block_on(buf.read_until(b'3', &mut v)).unwrap(), 0);
    assert_eq!(v, []);
}
