use futures::executor::block_on;
use futures::future::Future;
use futures::io::AsyncBufReadExt;
use futures::task::Poll;
use futures_test::io::AsyncReadTestExt;
use futures_test::task::noop_context;
use std::io::Cursor;
use std::pin::Pin;

#[test]
fn read_line() {
    let mut buf = Cursor::new(b"12");
    let mut v = String::new();
    assert_eq!(block_on(buf.read_line(&mut v)).unwrap(), 2);
    assert_eq!(v, "12");

    let mut buf = Cursor::new(b"12\n\n");
    let mut v = String::new();
    assert_eq!(block_on(buf.read_line(&mut v)).unwrap(), 3);
    assert_eq!(v, "12\n");
    v.clear();
    assert_eq!(block_on(buf.read_line(&mut v)).unwrap(), 1);
    assert_eq!(v, "\n");
    v.clear();
    assert_eq!(block_on(buf.read_line(&mut v)).unwrap(), 0);
    assert_eq!(v, "");
}

fn run<F: Future + Unpin>(mut f: F) -> F::Output {
    let mut cx = noop_context();
    loop {
        if let Poll::Ready(x) = Pin::new(&mut f).poll(&mut cx) {
            return x;
        }
    }
}

#[test]
fn maybe_pending() {
    let mut buf = b"12".interleave_pending();
    let mut v = String::new();
    assert_eq!(run(buf.read_line(&mut v)).unwrap(), 2);
    assert_eq!(v, "12");

    let mut buf = b"12\n\n".interleave_pending();
    let mut v = String::new();
    assert_eq!(run(buf.read_line(&mut v)).unwrap(), 3);
    assert_eq!(v, "12\n");
    v.clear();
    assert_eq!(run(buf.read_line(&mut v)).unwrap(), 1);
    assert_eq!(v, "\n");
    v.clear();
    assert_eq!(run(buf.read_line(&mut v)).unwrap(), 0);
    assert_eq!(v, "");
    v.clear();
    assert_eq!(run(buf.read_line(&mut v)).unwrap(), 0);
    assert_eq!(v, "");
}
