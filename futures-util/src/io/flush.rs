use std::io;

use {Poll, Future, Async, task};

use futures_io::AsyncWrite;

/// A future used to fully flush an I/O object.
///
/// Resolves to the underlying I/O object once the flush operation is complete.
///
/// Created by the [`flush`] function.
///
/// [`flush`]: fn.flush.html
#[derive(Debug)]
pub struct Flush<A> {
    a: Option<A>,
}

pub fn flush<A>(a: A) -> Flush<A>
    where A: AsyncWrite,
{
    Flush {
        a: Some(a),
    }
}

impl<A> Future for Flush<A>
    where A: AsyncWrite,
{
    type Item = A;
    type Error = io::Error;

    fn poll(&mut self, cx: &mut task::Context) -> Poll<A, io::Error> {
        try_ready!(self.a.as_mut().unwrap().poll_flush(cx));
        Ok(Async::Ready(self.a.take().unwrap()))
    }
}

