use std::vec::Vec;

use stream::Stream;
use stream::Fuse;
use poll::{Async, Poll};

/// An adapter that batches elements in a vector.
///
/// This adaptor will buffer up a list of items in the stream and pass on the
/// vector used for buffering when a specified capacity has been reached
/// or when underlying stream is not ready. This is created by
/// the `Stream::batched` method.
#[must_use = "streams do nothing unless polled"]
pub struct Batched<S>
    where S: Stream
{
    capacity: usize,
    stream: Fuse<S>,
    // queued error
    error: Option<S::Error>,
}

pub fn new<S>(stream: S, capacity: usize) -> Batched<S>
    where S: Stream
{
    assert!(capacity > 0);

    Batched {
        capacity: capacity,
        stream: stream.fuse(),
        error: None,
    }
}

impl<S> Stream for Batched<S>
    where S: Stream
{
    type Item = Vec<S::Item>;
    type Error = S::Error;

    fn poll(&mut self) -> Poll<Option<Vec<S::Item>>, S::Error> {
        if let Some(e) = self.error.take() {
            return Err(e);
        }

        let mut r = Vec::new();
        loop {
            match self.stream.poll() {
                Err(e) => {
                    if r.is_empty() {
                        return Err(e);
                    } else {
                        self.error = Some(e);
                        return Ok(Async::Ready(Some(r)));
                    }
                }
                Ok(Async::Ready(None)) => {
                    if r.is_empty() {
                        return Ok(Async::Ready(None));
                    } else {
                        return Ok(Async::Ready(Some(r)));
                    }
                }
                Ok(Async::Ready(Some(i))) => {
                    r.push(i);
                    if r.len() == self.capacity {
                        return Ok(Async::Ready(Some(r)));
                    }
                }
                Ok(Async::NotReady) => {
                    if r.is_empty() {
                        return Ok(Async::NotReady);
                    } else {
                        return Ok(Async::Ready(Some(r)));
                    }
                }
            }
        }
    }
}

