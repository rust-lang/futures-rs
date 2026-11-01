use futures::future::FutureExt;
use futures::task::Poll;
use futures_core::task::Context;
use futures_executor::block_on;
use futures_io::ErrorKind;
use futures_test::future::FutureTestExt;
use futures_test::task::{new_count_waker, panic_context};
use futures_util::io::{pipe, PipeReader, PipeWriter};
use futures_util::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt};
use static_assertions::assert_impl_all;

trait PollExt<T> {
    fn expect_pending(self);
    fn expect_ready(self) -> T;
}

impl<T> PollExt<T> for Poll<T> {
    #[track_caller]
    fn expect_pending(self) {
        assert!(self.is_pending());
    }

    #[track_caller]
    fn expect_ready(self) -> T {
        match self {
            Poll::Ready(v) => v,
            Poll::Pending => panic!("should be ready"),
        }
    }
}

// They have only `Pin<&mut Self>` methods. `&Self` can do nothing. Thus Sync.
assert_impl_all!(PipeReader: Send, Sync, Unpin);
assert_impl_all!(PipeWriter: Send, Sync, Unpin);

#[test]
fn small_write_nonblocking() {
    let (mut w, mut r) = pipe(8);
    let mut cx = panic_context();
    for _ in 0..10 {
        let mut buf = [0u8; 10];
        assert_eq!(w.write(b"12345").poll_unpin(&mut cx).expect_ready().unwrap(), 5);
        assert_eq!(r.read(&mut buf).poll_unpin(&mut cx).expect_ready().unwrap(), 5);
        assert_eq!(&buf[..5], b"12345");
    }
}

#[test]
fn big_write_nonblocking() {
    let (mut w, mut r) = pipe(7);
    let mut cx = panic_context();
    for _ in 0..10 {
        let mut buf = [0u8; 10];
        assert_eq!(w.write(b"1234567890").poll_unpin(&mut cx).expect_ready().unwrap(), 7);
        assert_eq!(r.read(&mut buf).poll_unpin(&mut cx).expect_ready().unwrap(), 7);
        assert_eq!(&buf[..7], b"1234567");
    }
}

#[test]
fn reader_blocked() {
    let (mut w, mut r) = pipe(8);
    let (waker, cnt) = new_count_waker();
    let mut cx = Context::from_waker(&waker);

    let mut buf = [0u8; 5];
    r.read(&mut buf).poll_unpin(&mut cx).expect_pending();
    assert_eq!(cnt.get(), 0);
    assert_eq!(w.write(b"12345").poll_unpin(&mut cx).expect_ready().unwrap(), 5);
    assert_eq!(cnt.get(), 1);
    assert_eq!(r.read(&mut buf[..3]).poll_unpin(&mut cx).expect_ready().unwrap(), 3);
    assert_eq!(&buf[..3], b"123");
    assert_eq!(cnt.get(), 1);
    assert_eq!(r.read(&mut buf[..3]).poll_unpin(&mut cx).expect_ready().unwrap(), 2);
    assert_eq!(&buf[..2], b"45");
    assert_eq!(cnt.get(), 1);
    r.read(&mut buf[..3]).poll_unpin(&mut cx).expect_pending();
    assert_eq!(cnt.get(), 1);
}

#[test]
fn writer_blocked() {
    let (mut w, mut r) = pipe(7);
    let (waker, cnt) = new_count_waker();
    let mut cx = Context::from_waker(&waker);
    let mut buf = [0u8; 10];

    assert_eq!(w.write(b"12345").poll_unpin(&mut cx).expect_ready().unwrap(), 5);
    assert_eq!(w.write(b"67890").poll_unpin(&mut cx).expect_ready().unwrap(), 2);
    assert_eq!(cnt.get(), 0);
    w.write(b"xxx").poll_unpin(&mut cx).expect_pending();
    assert_eq!(cnt.get(), 0);
    assert_eq!(r.read(&mut buf[..3]).poll_unpin(&mut cx).expect_ready().unwrap(), 3);
    assert_eq!(&buf[..3], b"123");
    assert_eq!(cnt.get(), 1);
    assert_eq!(w.write(b"abcde").poll_unpin(&mut cx).expect_ready().unwrap(), 3);
    assert_eq!(cnt.get(), 1);
    w.write(b"xxx").poll_unpin(&mut cx).expect_pending();
    assert_eq!(cnt.get(), 1);
    assert_eq!(r.read(&mut buf).poll_unpin(&mut cx).expect_ready().unwrap(), 7);
    assert_eq!(&buf[..7], b"4567abc");
    assert_eq!(cnt.get(), 2);
}

#[test]
fn reader_closed_notify_writer() {
    let (mut w, r) = pipe(4);
    let (waker, cnt) = new_count_waker();
    let mut cx = Context::from_waker(&waker);

    assert_eq!(cnt.get(), 0);
    assert_eq!(w.write(b"1234").poll_unpin(&mut cx).expect_ready().unwrap(), 4);
    w.write(b"xxx").poll_unpin(&mut cx).expect_pending();
    assert_eq!(cnt.get(), 0);
    drop(r);
    assert_eq!(cnt.get(), 1);

    assert_eq!(
        w.write(b"xxx").poll_unpin(&mut cx).expect_ready().unwrap_err().kind(),
        ErrorKind::BrokenPipe
    );
}

#[test]
fn writer_closed_notify_reader() {
    let (w, mut r) = pipe(4);
    let (waker, cnt) = new_count_waker();
    let mut cx = Context::from_waker(&waker);
    let mut buf = [0u8; 10];

    assert_eq!(cnt.get(), 0);
    r.read(&mut buf).poll_unpin(&mut cx).expect_pending();
    assert_eq!(cnt.get(), 0);
    drop(w);
    assert_eq!(cnt.get(), 1);

    assert_eq!(r.read(&mut [0u8; 10]).poll_unpin(&mut cx).expect_ready().unwrap(), 0);
}

#[test]
fn writer_closed_with_data() {
    let (mut w, mut r) = pipe(4);
    let mut cx = panic_context();
    let mut buf = [0u8; 10];

    assert_eq!(w.write(b"1234").poll_unpin(&mut cx).expect_ready().unwrap(), 4);
    drop(w);
    assert_eq!(r.read(&mut buf).poll_unpin(&mut cx).expect_ready().unwrap(), 4);
    assert_eq!(&buf[..4], b"1234");
    assert_eq!(r.read(&mut [0u8; 10]).poll_unpin(&mut cx).expect_ready().unwrap(), 0);
}

#[test]
fn smoke() {
    let (mut w, mut r) = pipe(128);
    let data = "hello world".repeat(1024);

    let reader = std::thread::spawn(|| {
        block_on(async move {
            let mut buf = String::new();
            r.read_to_string(&mut buf).interleave_pending().await.unwrap();
            buf
        })
    });

    let writer = std::thread::spawn({
        let data = data.clone();
        || {
            block_on(async move {
                w.write_all(data.as_bytes()).interleave_pending().await.unwrap();
            });
        }
    });

    writer.join().unwrap();
    let ret = reader.join().unwrap();
    assert_eq!(ret, data);
}

#[test]
fn smoke_bufread() {
    let (mut w, mut r) = pipe(128);
    let data = "hello world\n".repeat(1024);

    let reader = std::thread::spawn(|| {
        block_on(async move {
            let mut buf = String::new();
            while r.read_line(&mut buf).await.unwrap() != 0 {}
            buf
        })
    });

    let writer = std::thread::spawn({
        let data = data.clone();
        || {
            block_on(async move {
                w.write_all(data.as_bytes()).interleave_pending().await.unwrap();
            });
        }
    });

    writer.join().unwrap();
    let ret = reader.join().unwrap();
    assert_eq!(ret, data);
}
