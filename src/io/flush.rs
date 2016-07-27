use std::io;

use {Poll, Task, Future};
use io::WriteTask;

pub struct Flush<A> {
    a: Option<A>,
}

pub fn flush<A>(a: A) -> Flush<A>
    where A: WriteTask,
{
    Flush {
        a: Some(a),
    }
}

impl<A> Future for Flush<A>
    where A: WriteTask,
{
    type Item = A;
    type Error = io::Error;

    fn poll(&mut self, task: &mut Task) -> Poll<A, io::Error> {
        match self.a.as_mut().unwrap().flush(task) {
            Ok(()) => Poll::Ok(self.a.take().unwrap()),
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                Poll::NotReady
            }
            Err(e) => Poll::Err(e),
        }
    }

    fn schedule(&mut self, task: &mut Task) {
        if let Some(ref mut a) = self.a {
            a.schedule(task);
        }
    }
}

