use std::mem;
use std::prelude::v1::*;

use {Async, Poll};
use stream::{Stream, Fuse};

/// An adaptor that buffers elements in a vector before passing them on as one item.
///
/// This adaptor will buffer up a list of items in a stream and pass on the
/// vector used for buffering when a specified capacity has been reached. This is
/// created by the `Stream::buffer` method.
#[must_use = "streams do nothing unless polled"]
pub struct Buffer<S>
    where S: Stream
{
    capacity: usize, // TODO: Do we need this? Doesn't Vec::capacity() suffice?
    items: Vec<<S as Stream>::Item>,
    stream: Fuse<S>
}

pub fn new<S>(s: S, capacity: usize) -> Buffer<S>
    where S: Stream
{
    assert!(capacity > 0);
    
    Buffer {
        capacity: capacity,
        items: Vec::with_capacity(capacity),
        stream: super::fuse::new(s),
    }
}

impl<S> Stream for Buffer<S>
    where S: Stream
{
    type Item = Vec<<S as Stream>::Item>;
    type Error = <S as Stream>::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        loop {
            if let Some(item) = try_ready!(self.stream.poll()) {
                // Push the item into the buffer and check whether it is
                // full. If so, replace our buffer with a new and empty one
                // and return the full one.
                self.items.push(item);
                if self.items.len() >= self.capacity {
                    let full_buf = mem::replace(&mut self.items, Vec::with_capacity(self.capacity));
                    return Ok(Async::Ready(Some(full_buf)))
                }
            } else {
                // Since the underlying stream ran out of values, return
                // what we have buffered, if we have anything.
                return if self.items.len() > 0 {
                    let full_buf = mem::replace(&mut self.items, Vec::new());
                    Ok(Async::Ready(Some(full_buf)))
                } else {
                    Ok(Async::Ready(None))
                }
            }
        }
    }
}
