use std::io::{self, Write};

use futures::{Poll, Future};

/// A future used to fully flush an I/O object.
///
/// Resolves to the underlying I/O object once the flush operation is complete.
///
/// Created by the `flush` function.
pub struct Flush<A> {
    a: Option<A>,
}

/// Creates a future which will entirely flush an I/O object and then yield the
/// object itself.
///
/// This function will consume the object provided if an error happens, and
/// otherwise it will repeatedly call `flush` until it sees `Ok(())`, scheduling
/// a retry if `WouldBlock` is seen along the way.
pub fn flush<A>(a: A) -> Flush<A>
    where A: Write + 'static,
{
    Flush {
        a: Some(a),
    }
}

impl<A> Future for Flush<A>
    where A: Write + 'static,
{
    type Item = A;
    type Error = io::Error;

    fn poll(&mut self) -> Poll<A, io::Error> {
        match self.a.as_mut().unwrap().flush() {
            Ok(()) => Poll::Ok(self.a.take().unwrap()),
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                Poll::NotReady
            }
            Err(e) => Poll::Err(e),
        }
    }
}

