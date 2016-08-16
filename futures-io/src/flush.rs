use std::io;

use futures::{Poll, Task, Future};

use WriteTask;

/// A future used to fully flush an I/O object.
///
/// Resolves to the underlying I/O object once the flush operation is complete.
///
/// Created by the `flush` function.
pub struct Flush<A> {
    a: Option<A>,
    first: bool,
}

/// Creates a future which will entirely flush an I/O object and then yield the
/// object itself.
///
/// This function will consume the object provided if an error happens, and
/// otherwise it will repeatedly call `flush` until it sees `Ok(())`, scheduling
/// a retry if `WouldBlock` is seen along the way.
pub fn flush<A>(a: A) -> Flush<A>
    where A: WriteTask,
{
    Flush {
        a: Some(a),
        first: true,
    }
}

impl<A> Future for Flush<A>
    where A: WriteTask,
{
    type Item = A;
    type Error = io::Error;

    fn poll(&mut self, task: &mut Task) -> Poll<A, io::Error> {
        if self.first {
            self.first = false;
        } else {
            match try_poll!(self.a.as_mut().unwrap().poll(task)) {
                Ok(Some(ref r)) if r.is_write() => {}
                Ok(Some(_)) => return Poll::NotReady,
                Ok(None) => panic!("need flush but can't write"),
                Err(e) => return Poll::Err(e)
            }
        }
        match self.a.as_mut().unwrap().flush(task) {
            Ok(()) => Poll::Ok(self.a.take().unwrap()),
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                Poll::NotReady
            }
            Err(e) => Poll::Err(e),
        }
    }
}

