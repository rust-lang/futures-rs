extern crate futures;

use futures::executor::block_on;
use futures::prelude::*;
use futures::stream::iter_ok;

struct Join<T, U>(T, U);

impl<T: Stream, U> Stream for Join<T, U> {
    type Item = T::Item;
    type Error = T::Error;

    fn poll_next(&mut self, cx: &mut task::Context) -> Poll<Option<T::Item>, T::Error> {
        self.0.poll_next(cx)
    }
}

impl<T, U: Sink> Sink for Join<T, U> {
    type SinkItem = U::SinkItem;
    type SinkError = U::SinkError;

    fn poll_ready(&mut self, cx: &mut task::Context) -> Poll<(), Self::SinkError> {
        self.1.poll_ready(cx)
    }

    fn start_send(&mut self, item: Self::SinkItem) -> Result<(), Self::SinkError> {
        self.1.start_send(item)
    }

    fn poll_flush(&mut self, cx: &mut task::Context) -> Poll<(), Self::SinkError> {
        self.1.poll_flush(cx)
    }

    fn poll_close(&mut self, cx: &mut task::Context) -> Poll<(), Self::SinkError> {
        self.1.poll_close(cx)
    }
}

#[test]
fn test_split() {
    let mut dest = Vec::new();
    {
        let j = Join(iter_ok(vec![10, 20, 30]), &mut dest);
        let (sink, stream) = j.split();
        let j = sink.reunite(stream).expect("test_split: reunite error");
        let (sink, stream) = j.split();
        block_on(sink.send_all(stream)).unwrap();
    }
    assert_eq!(dest, vec![10, 20, 30]);
}
