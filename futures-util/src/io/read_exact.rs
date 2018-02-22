use std::io;
use std::mem;

use {Poll, Future, task};

use io::AsyncRead;

/// A future which can be used to easily read exactly enough bytes to fill
/// a buffer.
///
/// Created by the [`read_exact`] function.
///
/// [`read_exact`]: fn.read_exact.html
#[derive(Debug)]
pub struct ReadExact<A, T> {
    state: State<A, T>,
}

#[derive(Debug)]
enum State<A, T> {
    Reading {
        a: A,
        buf: T,
        pos: usize,
    },
    Empty,
}

pub fn read_exact<A, T>(a: A, buf: T) -> ReadExact<A, T>
    where A: AsyncRead,
          T: AsMut<[u8]>,
{
    ReadExact {
        state: State::Reading {
            a,
            buf,
            pos: 0,
        },
    }
}

fn eof() -> io::Error {
    io::Error::new(io::ErrorKind::UnexpectedEof, "early eof")
}

impl<A, T> Future for ReadExact<A, T>
    where A: AsyncRead,
          T: AsMut<[u8]>,
{
    type Item = (A, T);
    type Error = io::Error;

    fn poll(&mut self, cx: &mut task::Context) -> Poll<(A, T), io::Error> {
        match self.state {
            State::Reading { ref mut a, ref mut buf, ref mut pos } => {
                let buf = buf.as_mut();
                while *pos < buf.len() {
                    let n = try_ready!(a.poll_read(cx, &mut buf[*pos..]));
                    *pos += n;
                    if n == 0 {
                        return Err(eof())
                    }
                }
            }
            State::Empty => panic!("poll a ReadExact after it's done"),
        }

        match mem::replace(&mut self.state, State::Empty) {
            State::Reading { a, buf, .. } => Ok((a, buf).into()),
            State::Empty => panic!(),
        }
    }
}
