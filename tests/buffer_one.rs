extern crate futures;

use futures::{Sink, StartSend, Poll, Async, AsyncSink};
use futures::sink::BufferOne;
use std::collections::VecDeque;

#[test]
fn always_ready() {
    let mut sink = BufferOne::new(
        Mock::default()
            .expect_start_send(1, Ok(AsyncSink::Ready))
            .expect_poll_complete(Ok(Async::Ready(())))
            .expect_start_send(2, Ok(AsyncSink::Ready))
            .expect_start_send(3, Ok(AsyncSink::Ready))
            .expect_poll_complete(Ok(Async::Ready(())))
            );

    assert!(sink.start_send(1).unwrap().is_ready());
    assert_eq!(5, sink.get_ref().len());

    assert!(sink.poll_complete().unwrap().is_ready());
    assert_eq!(3, sink.get_ref().len());

    assert!(sink.start_send(2).unwrap().is_ready());
    assert!(sink.start_send(3).unwrap().is_ready());
    assert_eq!(2, sink.get_ref().len());

    assert!(sink.poll_complete().unwrap().is_ready());
    assert_eq!(0, sink.get_ref().len());
}

#[test]
fn start_send_push_back() {
    let mut sink = BufferOne::new(
        Mock::default()
            .expect_start_send(1, Ok(AsyncSink::NotReady(1)))
            .expect_poll_complete(Ok(Async::Ready(())))
            .expect_start_send(1, Ok(AsyncSink::Ready))
            .expect_poll_complete(Ok(Async::Ready(())))
            );

    assert!(sink.start_send(1).unwrap().is_ready());
    assert_eq!(4, sink.get_ref().len());

    assert!(!sink.poll_complete().unwrap().is_ready());
    assert_eq!(2, sink.get_ref().len());

    assert!(sink.poll_complete().unwrap().is_ready());
    assert_eq!(0, sink.get_ref().len());
}

#[test]
fn poll_complete_push_back() {
    let mut sink = BufferOne::new(
        Mock::default()
            .expect_start_send(1, Ok(AsyncSink::Ready))
            .expect_poll_complete(Ok(Async::NotReady))
            .expect_start_send(2, Ok(AsyncSink::Ready))
            .expect_poll_complete(Ok(Async::Ready(())))
            );

    assert!(sink.start_send(1).unwrap().is_ready());
    assert_eq!(4, sink.get_ref().len());

    assert!(!sink.poll_complete().unwrap().is_ready());
    assert_eq!(2, sink.get_ref().len());

    assert!(sink.start_send(2).unwrap().is_ready());
    assert_eq!(2, sink.get_ref().len());

    assert!(sink.poll_complete().unwrap().is_ready());
    assert_eq!(0, sink.get_ref().len());
}

#[test]
fn poll_ready() {
    let mut sink = BufferOne::new(
        Mock::default()
            .expect_start_send(1, Ok(AsyncSink::Ready))
            .expect_start_send(2, Ok(AsyncSink::NotReady(2)))
            .expect_poll_complete(Ok(Async::NotReady))
            .expect_start_send(2, Ok(AsyncSink::Ready))
            );

    assert!(sink.poll_ready().is_ready());
    assert_eq!(4, sink.get_ref().len());

    assert!(sink.start_send(1).unwrap().is_ready());
    assert_eq!(4, sink.get_ref().len());

    assert!(sink.poll_ready().is_ready());
    assert_eq!(3, sink.get_ref().len());

    assert!(sink.start_send(2).unwrap().is_ready());
    assert!(!sink.poll_ready().is_ready());
    assert_eq!(1, sink.get_ref().len());

    assert!(sink.poll_ready().is_ready());
    assert_eq!(0, sink.get_ref().len());
}

#[test]
fn poll_ready_err() {
    let mut sink = BufferOne::new(
        Mock::default()
            .expect_start_send(1, Err(()))
            );

    assert!(sink.start_send(1).unwrap().is_ready());

    assert!(sink.poll_ready().is_ready());
    assert_eq!(0, sink.get_ref().len());

    assert!(sink.start_send(2).is_err());
}

#[derive(Default)]
struct Mock {
    expect: VecDeque<Call>,
}

enum Call {
    StartSend(u32, StartSend<u32, ()>),
    PollComplete(Poll<(), ()>),
}

impl Mock {
    fn expect_start_send(mut self, val: u32, ret: StartSend<u32, ()>) -> Self {
        self.expect.push_back(Call::StartSend(val, ret));
        self
    }

    fn expect_poll_complete(mut self, ret: Poll<(), ()>) -> Self {
        self.expect.push_back(Call::PollComplete(ret));
        self
    }

    fn len(&self) -> usize {
        self.expect.len()
    }
}

impl Sink for Mock {
    type SinkItem = u32;
    type SinkError = ();

    fn start_send(&mut self, item: u32) -> StartSend<u32, ()> {
        match self.expect.pop_front() {
            Some(Call::StartSend(v, ret)) => {
                assert_eq!(item, v);
                ret
            }
            Some(Call::PollComplete(..)) => panic!("expected poll_complete but got start_send"),
            None => panic!("expected no further calls"),
        }
    }

    fn poll_complete(&mut self) -> Poll<(), ()> {
        match self.expect.pop_front() {
            Some(Call::PollComplete(ret)) => ret,
            Some(Call::StartSend(..)) => panic!("expected start_send but got poll_complete"),
            None => panic!("expected no further calls"),
        }
    }

    fn close(&mut self) -> Poll<(), ()> {
        self.poll_complete()
    }
}
