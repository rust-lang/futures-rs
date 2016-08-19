use std::io::{self, Read};
use std::mem;

use futures::{Poll, Future};

/// A future which can be used to easily read the entire contents of a stream
/// into a vector.
///
/// Created by the `read_to_end` function.
pub struct ReadToEnd<A> {
    a: A,
    buf: Vec<u8>,
}

/// Creates a future which will read all the bytes associated with the I/O
/// object `A` into the buffer provided.
///
/// In the case of an error the buffer and the object will be discarded, with
/// the error yielded. In the case of success the object will be destroyed and
/// the buffer will be returned, with all data read from the stream appended to
/// the buffer.
pub fn read_to_end<A>(a: A, buf: Vec<u8>) -> ReadToEnd<A>
    where A: Read,
{
    ReadToEnd {
        a: a,
        buf: buf,
    }
}

impl<A> Future for ReadToEnd<A>
    where A: Read,
{
    type Item = Vec<u8>;
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Vec<u8>, io::Error> {
        // If we get `Ok`, then we know the stream hit EOF and we're done. If we
        // hit "would block" then all the read data so far is in our buffer, and
        // otherwise we propagate errors.
        try_nb!(self.a.read_to_end(&mut self.buf));
        Poll::Ok(mem::replace(&mut self.buf, Vec::new()))
    }
}
