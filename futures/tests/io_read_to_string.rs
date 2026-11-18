use futures::{
    executor::block_on,
    future::{Future, FutureExt},
    io::{AsyncReadExt, Cursor},
    stream::{self, StreamExt, TryStreamExt},
    task::Poll,
};
use futures_test::{io::AsyncReadTestExt, task::noop_context};

#[test]
fn read_to_string() {
    let mut c = Cursor::new(&b""[..]);
    let mut v = String::new();
    assert_eq!(block_on(c.read_to_string(&mut v)).unwrap(), 0);
    assert_eq!(v, "");

    let mut c = Cursor::new(&b"1"[..]);
    let mut v = String::new();
    assert_eq!(block_on(c.read_to_string(&mut v)).unwrap(), 1);
    assert_eq!(v, "1");

    let mut c = Cursor::new(&b"\xff"[..]);
    let mut v = String::new();
    assert!(block_on(c.read_to_string(&mut v)).is_err());
}

#[test]
fn read_to_string_utf8_error() {
    let mut c = Cursor::new(&b"12\xff"[..]);
    let mut v = String::from("abc");
    let res = block_on(c.read_to_string(&mut v));
    assert_eq!(res.unwrap_err().kind(), std::io::ErrorKind::InvalidData);
    assert_eq!(v, "abc");
}

#[test]
fn read_to_string_drop() {
    // string contents should be preserved if the future is dropped
    let mut c = Cursor::new(&b"12"[..]).interleave_pending();
    let mut v = String::from("abc");
    let mut f = c.read_to_string(&mut v);
    assert!(f.poll_unpin(&mut noop_context()).is_pending());
    drop(f);
    assert_eq!(v, "abc");
}

#[test]
fn interleave_pending() {
    fn run<F: Future + Unpin>(mut f: F) -> F::Output {
        let mut cx = noop_context();
        loop {
            if let Poll::Ready(x) = f.poll_unpin(&mut cx) {
                return x;
            }
        }
    }
    let mut buf = stream::iter(vec![&b"12"[..], &b"33"[..], &b"3"[..]])
        .map(Ok)
        .into_async_read()
        .interleave_pending();

    let mut v = String::new();
    assert_eq!(run(buf.read_to_string(&mut v)).unwrap(), 5);
    assert_eq!(v, "12333");
}
