#![feature(pin, arbitrary_self_types, futures_api)]

use futures::executor::block_on;
use futures::sink::{Sink, SinkExt};
use futures::stream::{self, Stream, StreamExt};
use futures::task::{self, Poll};
use pin_utils::unsafe_pinned;
use std::pin::Pin;

struct Join<T, U> {
    stream: T,
    sink: U
}

impl<T, U> Join<T, U> {
    unsafe_pinned!(stream: T);
    unsafe_pinned!(sink: U);
}

impl<T: Stream, U> Stream for Join<T, U> {
    type Item = T::Item;

    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context,
    ) -> Poll<Option<T::Item>> {
        self.stream().poll_next(cx)
    }
}

impl<T, U: Sink> Sink for Join<T, U> {
    type SinkItem = U::SinkItem;
    type SinkError = U::SinkError;

    fn poll_ready(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context,
    ) -> Poll<Result<(), Self::SinkError>> {
        self.sink().poll_ready(cx)
    }

    fn start_send(
        mut self: Pin<&mut Self>,
        item: Self::SinkItem,
    ) -> Result<(), Self::SinkError> {
        self.sink().start_send(item)
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context,
    ) -> Poll<Result<(), Self::SinkError>> {
        self.sink().poll_flush(cx)
    }

    fn poll_close(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context,
    ) -> Poll<Result<(), Self::SinkError>> {
        self.sink().poll_close(cx)
    }
}

#[test]
fn test_split() {
    let mut dest: Vec<i32> = Vec::new();
    {
       let join = Join {
            stream: stream::iter(vec![10, 20, 30]),
            sink: &mut dest
        };

        let (sink, stream) = join.split();
        let join = sink.reunite(stream).expect("test_split: reunite error");
        let (mut sink, mut stream) = join.split();
        block_on(sink.send_all(&mut stream)).unwrap();
    }
    assert_eq!(dest, vec![10, 20, 30]);
}
