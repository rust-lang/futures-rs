use core::pin::Pin;
use futures::future;
use futures::io::{AsyncRead, AsyncBufRead};
use futures::stream::{self, IntoAsyncReadParts, StreamExt, TryStreamExt};
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
    let mut reader = if let IntoAsyncReadParts::PartiallyBuffered {
        mut chunk,
        offset,
        stream: rest,
    } = reader.into_inner()
    {
        assert_eq!(&chunk, &[1, 2, 3, 4, 5]);
        assert_eq!(offset, 3);

        // package the stream back up by splitting off the chunk according to the offset we got back
        let stream = stream::once(future::ready(Ok(chunk.split_off(offset)))).chain(rest);
        stream.into_async_read()
    } else {
        panic!("reader should have a partial buffer");
    };

    // resume reading as normal
    assert_read!(reader, &mut buf, 2);
    assert_eq!(&buf[..2], &[4, 5]);

    assert_read!(reader, &mut buf, 3);
    assert_eq!(&buf, &[1, 2, 3]);

    assert_read!(reader, &mut buf, 2);
    assert_eq!(&buf[..2], &[4, 5]);

    let mut reader = if let IntoAsyncReadParts::Pending { stream: rest } = reader.into_inner() {
        // package the stream back up and resume reading as normal
        rest.into_async_read()
    } else {
        panic!("reader should have no partial buffer on a chunk boundary");
    };

    assert_read!(reader, &mut buf, 3);
    assert_eq!(&buf, &[1, 2, 3]);

    assert_read!(reader, &mut buf, 2);
    assert_eq!(&buf[..2], &[4, 5]);

    assert_read!(reader, &mut buf, 0);

    // at the end of the stream, we should see Eof
    if let IntoAsyncReadParts::Eof = reader.into_inner() {
        // succeed
    } else {
        panic!("reader should be at eof");
    }
}
