use core::pin::Pin;
use futures::future;
use futures::io::{AsyncRead, AsyncBufRead};
use futures::stream::{self, StreamExt, TryStreamExt};
use futures::task::Poll;
use futures_test::{task::noop_context, stream::StreamTestExt};

macro_rules! assert_read {
    ($reader:expr, $buf:expr, $item:expr) => {
        let mut cx = noop_context();
        loop {
            match Pin::new(&mut $reader).poll_read(&mut cx, $buf) {
                Poll::Ready(Ok(x)) => {
                    assert_eq!(x, $item);
                    break;
                }
                Poll::Ready(Err(err)) => {
                    panic!("assertion failed: expected value but got {}", err);
                }
                Poll::Pending => {
                    continue;
                }
            }
        }
    };
}

macro_rules! assert_fill_buf {
    ($reader:expr, $buf:expr) => {
        let mut cx = noop_context();
        loop {
            match Pin::new(&mut $reader).poll_fill_buf(&mut cx) {
                Poll::Ready(Ok(x)) => {
                    assert_eq!(x, $buf);
                    break;
                }
                Poll::Ready(Err(err)) => {
                    panic!("assertion failed: expected value but got {}", err);
                }
                Poll::Pending => {
                    continue;
                }
            }
        }
    };
}

#[test]
fn test_into_async_read() {
    let stream = stream::iter((1..=3).flat_map(|_| vec![Ok(vec![]), Ok(vec![1, 2, 3, 4, 5])]));
    let mut reader = stream.interleave_pending().into_async_read();
    let mut buf = vec![0; 3];

    assert_read!(reader, &mut buf, 3);
    assert_eq!(&buf, &[1, 2, 3]);

    assert_read!(reader, &mut buf, 2);
    assert_eq!(&buf[..2], &[4, 5]);

    assert_read!(reader, &mut buf, 3);
    assert_eq!(&buf, &[1, 2, 3]);

    assert_read!(reader, &mut buf, 2);
    assert_eq!(&buf[..2], &[4, 5]);

    assert_read!(reader, &mut buf, 3);
    assert_eq!(&buf, &[1, 2, 3]);

    assert_read!(reader, &mut buf, 2);
    assert_eq!(&buf[..2], &[4, 5]);

    assert_read!(reader, &mut buf, 0);
}

#[test]
fn test_into_async_bufread() -> std::io::Result<()> {
    let stream = stream::iter((1..=2).flat_map(|_| vec![Ok(vec![]), Ok(vec![1, 2, 3, 4, 5])]));
    let mut reader = stream.interleave_pending().into_async_read();

    let mut reader = Pin::new(&mut reader);

    assert_fill_buf!(reader, &[1, 2, 3, 4, 5][..]);
    reader.as_mut().consume(3);

    assert_fill_buf!(reader, &[4, 5][..]);
    reader.as_mut().consume(2);

    assert_fill_buf!(reader, &[1, 2, 3, 4, 5][..]);
    reader.as_mut().consume(2);

    assert_fill_buf!(reader, &[3, 4, 5][..]);
    reader.as_mut().consume(3);

    assert_fill_buf!(reader, &[][..]);

    Ok(())
}

#[test]
fn test_into_async_read_into_inner() {
    let stream = stream::iter((1..=3).flat_map(|_| vec![Ok(vec![]), Ok(vec![1, 2, 3, 4, 5])]));
    let mut reader = stream.interleave_pending().into_async_read();
    let mut buf = vec![0; 3];

    assert_read!(reader, &mut buf, 3);
    assert_eq!(&buf, &[1, 2, 3]);

    // turn the reader into its inner parts, which we can inspect
    let (first, rest) = reader.into_inner();
    let (mut chunk, offset) = first.expect(".into_inner() called in the middle of a chunk");
    assert_eq!(&chunk, &[1, 2, 3, 4, 5]);
    assert_eq!(offset, 3);

    // package the stream back up by splitting off the chunk according to the offset we got back
    let stream = stream::once(future::ready(Ok(chunk.split_off(offset)))).chain(rest);
    let mut reader = stream.into_async_read();

    // resume reading as normal
    assert_read!(reader, &mut buf, 2);
    assert_eq!(&buf[..2], &[4, 5]);

    assert_read!(reader, &mut buf, 3);
    assert_eq!(&buf, &[1, 2, 3]);

    assert_read!(reader, &mut buf, 2);
    assert_eq!(&buf[..2], &[4, 5]);

    // turn the reader into its inner parts again, this time on a chunk boundary
    let (first, rest) = reader.into_inner();
    assert!(first.is_none());

    // package the stream back up and resume reading as normal
    let mut reader = rest.into_async_read();

    assert_read!(reader, &mut buf, 3);
    assert_eq!(&buf, &[1, 2, 3]);

    assert_read!(reader, &mut buf, 2);
    assert_eq!(&buf[..2], &[4, 5]);

    assert_read!(reader, &mut buf, 0);

    // at the end of the stream, there should be no element buffered in the reader
    let (first, _end) = reader.into_inner();
    assert!(first.is_none());
}
