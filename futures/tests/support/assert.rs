use futures::stream::Stream;
use futures::task::Poll;
use std::fmt;
use std::mem::PinMut;

use super::{with_noop_waker_context, with_panic_waker_context};

pub fn assert_stream_pending<S: Stream>(stream: PinMut<S>) {
    with_noop_waker_context(|cx| {
        match stream.poll_next(cx) {
            Poll::Ready(_) => panic!("stream is not pending"),
            Poll::Pending => {},
        }
    })
}

pub fn assert_stream_next<S: Stream>(stream: PinMut<S>, item: S::Item)
    where S::Item: Eq + fmt::Debug
{
    with_panic_waker_context(|cx| {
        match stream.poll_next(cx) {
            Poll::Ready(Some(x)) => assert_eq!(x, item),
            Poll::Ready(None) => panic!("stream is at its end"),
            Poll::Pending => panic!("stream wasn't ready"),
        }
    })
}

pub fn assert_stream_done<S: Stream>(stream: PinMut<S>)
{
    with_panic_waker_context(|cx| {
        match stream.poll_next(cx) {
            Poll::Ready(Some(_)) => panic!("stream had more elements"),
            Poll::Ready(None) => {},
            Poll::Pending => panic!("stream wasn't ready"),
        }
    })
}
