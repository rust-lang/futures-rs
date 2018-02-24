use std::io;

use {Poll, Future, Async, task};

use futures_io::AsyncWrite;

/// A future used to fully close an I/O object.
///
/// Resolves to the underlying I/O object once the close operation is
/// complete.
///
/// Created by the [`close`] function.
///
/// [`close`]: fn.close.html
#[derive(Debug)]
pub struct Close<A> {
    a: Option<A>,
}

pub fn close<A>(a: A) -> Close<A>
    where A: AsyncWrite,
{
    Close {
        a: Some(a),
    }
}

impl<A> Future for Close<A>
    where A: AsyncWrite,
{
    type Item = A;
    type Error = io::Error;

    fn poll(&mut self, cx: &mut task::Context) -> Poll<A, io::Error> {
        try_ready!(self.a.as_mut().unwrap().poll_close(cx));
        Ok(Async::Ready(self.a.take().unwrap()))
    }
}
