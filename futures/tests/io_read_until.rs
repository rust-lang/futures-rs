use futures::executor::block_on;
use futures::future::Future;
use futures::io::{AsyncRead, AsyncBufRead, AsyncBufReadExt};
use futures::task::{Context, Poll};
use futures_test::task::noop_context;
use std::cmp;
use std::io::{self, Cursor};
use std::pin::Pin;

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

fn run<F: Future + Unpin>(mut f: F) -> F::Output {
    let mut cx = noop_context();
    loop {
        if let Poll::Ready(x) = Pin::new(&mut f).poll(&mut cx) {
            return x;
        }
    }
}

struct MaybePending<'a> {
    inner: &'a [u8],
    ready: bool,
}

impl<'a> MaybePending<'a> {
    fn new(inner: &'a [u8]) -> Self {
        Self { inner, ready: false }
    }
}

impl AsyncRead for MaybePending<'_> {
    fn poll_read(self: Pin<&mut Self>, _: &mut Context<'_>, _: &mut [u8])
        -> Poll<io::Result<usize>>
    {
        unimplemented!()
    }
}

impl AsyncBufRead for MaybePending<'_> {
    fn poll_fill_buf<'a>(mut self: Pin<&'a mut Self>, _: &mut Context<'_>)
        -> Poll<io::Result<&'a [u8]>>
    {
        if self.ready {
            self.ready = false;
            if self.inner.is_empty() { return Poll::Ready(Ok(&[])) }
            let len = cmp::min(2, self.inner.len());
            Poll::Ready(Ok(&self.inner[0..len]))
        } else {
            self.ready = true;
            Poll::Pending
        }
    }

    fn consume(mut self: Pin<&mut Self>, amt: usize) {
        self.inner = &self.inner[amt..];
    }
}

#[test]
fn maybe_pending() {
    let mut buf = MaybePending::new(b"12");
    let mut v = Vec::new();
    assert_eq!(run(buf.read_until(b'3', &mut v)).unwrap(), 2);
    assert_eq!(v, b"12");

    let mut buf = MaybePending::new(b"12333");
    let mut v = Vec::new();
    assert_eq!(run(buf.read_until(b'3', &mut v)).unwrap(), 3);
    assert_eq!(v, b"123");
    v.clear();
    assert_eq!(run(buf.read_until(b'3', &mut v)).unwrap(), 1);
    assert_eq!(v, b"3");
    v.clear();
    assert_eq!(run(buf.read_until(b'3', &mut v)).unwrap(), 1);
    assert_eq!(v, b"3");
    v.clear();
    assert_eq!(run(buf.read_until(b'3', &mut v)).unwrap(), 0);
    assert_eq!(v, []);
}
