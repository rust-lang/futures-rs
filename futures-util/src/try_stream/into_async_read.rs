use core::pin::Pin;
use futures_core::stream::TryStream;
use futures_core::task::{Context, Poll};
use futures_io::AsyncRead;
use std::cmp;
use std::io::{Error, Result};

/// An `AsyncRead` for the [`into_async_read`](super::TryStreamExt::into_async_read) combinator.
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct IntoAsyncRead<St>
where
    St: TryStream<Error = Error> + Unpin,
    St::Ok: AsRef<[u8]>,
{
    stream: St,
    state: ReadState<St::Ok>,
}

impl<St> Unpin for IntoAsyncRead<St>
where
    St: TryStream<Error = Error> + Unpin,
    St::Ok: AsRef<[u8]>,
{
}

#[derive(Debug)]
enum ReadState<T: AsRef<[u8]>> {
    Ready { chunk: T, chunk_start: usize },
    PendingChunk,
    Eof,
}

impl<St> IntoAsyncRead<St>
where
    St: TryStream<Error = Error> + Unpin,
    St::Ok: AsRef<[u8]>,
{
    pub(super) fn new(stream: St) -> Self {
        IntoAsyncRead {
            stream,
            state: ReadState::PendingChunk,
        }
    }
}

impl<St> AsyncRead for IntoAsyncRead<St>
where
    St: TryStream<Error = Error> + Unpin,
    St::Ok: AsRef<[u8]>,
{
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<Result<usize>> {
        loop {
            match &mut self.state {
                ReadState::Ready { chunk, chunk_start } => {
                    let chunk = chunk.as_ref();
                    let len = cmp::min(buf.len(), chunk.len() - *chunk_start);

                    buf[..len].copy_from_slice(
                        &chunk[*chunk_start..*chunk_start + len],
                    );
                    *chunk_start += len;

                    if chunk.len() == *chunk_start {
                        self.state = ReadState::PendingChunk;
                    }

                    return Poll::Ready(Ok(len));
                }
                ReadState::PendingChunk => {
                    match ready!(Pin::new(&mut self.stream).try_poll_next(cx)) {
                        Some(Ok(chunk)) => {
                            self.state = ReadState::Ready {
                                chunk,
                                chunk_start: 0,
                            };
                            continue;
                        }
                        Some(Err(err)) => {
                            self.state = ReadState::Eof;
                            return Poll::Ready(Err(err));
                        }
                        None => {
                            self.state = ReadState::Eof;
                            return Poll::Ready(Ok(0));
                        }
                    }
                }
                ReadState::Eof => {
                    return Poll::Ready(Ok(0));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::stream::{self, StreamExt, TryStreamExt};
    use futures_io::AsyncRead;
    use futures_test::task::noop_context;

    macro_rules! assert_read {
        ($reader:expr, $buf:expr, $item:expr) => {
            let mut cx = noop_context();
            match Pin::new(&mut $reader).poll_read(&mut cx, $buf) {
                Poll::Ready(Ok(x)) => {
                    assert_eq!(x, $item);
                }
                Poll::Ready(Err(err)) => {
                    panic!("assertion failed: expected value but got {}", err);
                }
                Poll::Pending => {
                    panic!("assertion failed: reader was not ready");
                }
            }
        };
    }

    #[test]
    fn test_into_async_read() {
        let stream = stream::iter(1..=3).map(|_| Ok(vec![1, 2, 3, 4, 5]));
        let mut reader = stream.into_async_read();
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
}
