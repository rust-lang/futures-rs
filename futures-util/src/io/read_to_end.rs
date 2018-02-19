use std::io::{self, ErrorKind};
use std::mem;
use std::vec::Vec;

use {Async, Poll, Future, task};

use io::AsyncRead;

/// A future which can be used to easily read the entire contents of a stream
/// into a vector.
///
/// Created by the [`read_to_end`] function.
///
/// [`read_to_end`]: fn.read_to_end.html
#[derive(Debug)]
pub struct ReadToEnd<A> {
    state: State<A>,
}

#[derive(Debug)]
enum State<A> {
    Reading {
        a: A,
        buf: Vec<u8>,
    },
    Empty,
}

/// Creates a future which will read all the bytes associated with the I/O
/// object `A` into the buffer provided.
///
/// In the case of an error the buffer and the object will be discarded, with
/// the error yielded. In the case of success the object will be destroyed and
/// the buffer will be returned, with all data read from the stream appended to
/// the buffer.
pub fn read_to_end<A>(a: A, buf: Vec<u8>) -> ReadToEnd<A>
    where A: AsyncRead,
{
    ReadToEnd {
        state: State::Reading {
            a: a,
            buf: buf,
        }
    }
}

struct Guard<'a> { buf: &'a mut Vec<u8>, len: usize }

impl<'a> Drop for Guard<'a> {
    fn drop(&mut self) {
        unsafe { self.buf.set_len(self.len); }
    }
}

// This uses an adaptive system to extend the vector when it fills. We want to
// avoid paying to allocate and zero a huge chunk of memory if the reader only
// has 4 bytes while still making large reads if the reader does have a ton
// of data to return. Simply tacking on an extra DEFAULT_BUF_SIZE space every
// time is 4,500 times (!) slower than this if the reader has a very small
// amount of data to return.
//
// Because we're extending the buffer with uninitialized data for trusted
// readers, we need to make sure to truncate that if any of this panics.
fn read_to_end_internal<R: AsyncRead + ?Sized>(r: &mut R, buf: &mut Vec<u8>, cx: &mut task::Context)
    -> Poll<usize, io::Error>
{
    let start_len = buf.len();
    let mut g = Guard { len: buf.len(), buf: buf };
    let ret;
    loop {
        if g.len == g.buf.len() {
            unsafe {
                g.buf.reserve(32);
                let capacity = g.buf.capacity();
                g.buf.set_len(capacity);
                r.initializer().initialize(&mut g.buf[g.len..]);
            }
        }

        match r.poll_read(&mut g.buf[g.len..], cx) {
            Ok(Async::Ready(0)) => {
                ret = Ok(Async::Ready(g.len - start_len));
                break;
            }
            Ok(Async::Ready(n)) => g.len += n,
            Ok(Async::Pending) => return Ok(Async::Pending),
            Err(ref e) if e.kind() == ErrorKind::Interrupted => {}
            Err(e) => {
                ret = Err(e);
                break;
            }
        }
    }

    ret
}

impl<A> Future for ReadToEnd<A>
    where A: AsyncRead,
{
    type Item = (A, Vec<u8>);
    type Error = io::Error;

    fn poll(&mut self, cx: &mut task::Context) -> Poll<(A, Vec<u8>), io::Error> {
        match self.state {
            State::Reading { ref mut a, ref mut buf } => {
                // If we get `Ok`, then we know the stream hit EOF and we're done. If we
                // hit "would block" then all the read data so far is in our buffer, and
                // otherwise we propagate errors
                try_ready!(read_to_end_internal(a, buf, cx));
            },
            State::Empty => panic!("poll ReadToEnd after it's done"),
        }

        match mem::replace(&mut self.state, State::Empty) {
            State::Reading { a, buf } => Ok((a, buf).into()),
            State::Empty => unreachable!(),
        }
    }
}
