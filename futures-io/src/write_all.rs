use std::io::{self, Write};
use std::mem;

use futures::{Poll, Future};

/// A future used to write the entire contents of some data to a stream.
///
/// This is created by the `write_all` top-level method.
pub struct WriteAll<A, T> {
    state: State<A, T>,
}

enum State<A, T> {
    Writing {
        a: A,
        buf: T,
        pos: usize,
    },
    Empty,
}

/// Creates a future that will write the entire contents of the buffer `buf` to
/// the stream `a` provided.
///
/// The returned future will not return until all the data has been written, and
/// the future will resolve to the stream as well as the buffer (for reuse if
/// needed).
///
/// Any error which happens during writing will cause both the stream and the
/// buffer to get destroyed.
///
/// The `buf` parameter here only requires the `AsRef<[u8]>` trait, which should
/// be broadly applicable to accepting data which can be converted to a slice.
/// The `Window` struct is also available in this crate to provide a different
/// window into a slice if necessary.
pub fn write_all<A, T>(a: A, buf: T) -> WriteAll<A, T>
    where A: Write + 'static,
          T: AsRef<[u8]> + 'static,
{
    WriteAll {
        state: State::Writing {
            a: a,
            buf: buf,
            pos: 0,
        },
    }
}

fn zero_write() -> io::Error {
    io::Error::new(io::ErrorKind::WriteZero, "zero-length write")
}

impl<A, T> Future for WriteAll<A, T>
    where A: Write + 'static,
          T: AsRef<[u8]> + 'static,
{
    type Item = (A, T);
    type Error = io::Error;

    fn poll(&mut self) -> Poll<(A, T), io::Error> {
        match self.state {
            State::Writing { ref mut a, ref buf, ref mut pos } => {
                let buf = buf.as_ref();
                while *pos < buf.len() {
                    match a.write(&buf[*pos..]) {
                        Ok(0) => return Poll::Err(zero_write()),
                        Ok(n) => *pos += n,
                        Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                            return Poll::NotReady
                        }
                        Err(e) => return Poll::Err(e),
                    }
                }
            }
            State::Empty => panic!("poll a WriteAll after it's done"),
        }

        match mem::replace(&mut self.state, State::Empty) {
            State::Writing { a, buf, .. } => Poll::Ok((a, buf)),
            State::Empty => panic!(),
        }
    }
}
