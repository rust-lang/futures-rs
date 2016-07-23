use {Task, Poll};
use stream::Stream;

/// A stream combinator which skips a number of elements before continuing.
///
/// This structure is produced by the `Stream::skip` method.
pub struct Skip<S> {
    stream: S,
    remaining: u64,
}

pub fn new<S>(s: S, amt: u64) -> Skip<S>
    where S: Stream,
{
    Skip {
        stream: s,
        remaining: amt,
    }
}

impl<S> Stream for Skip<S>
    where S: Stream,
{
    type Item = S::Item;
    type Error = S::Error;

    fn poll(&mut self, task: &mut Task) -> Poll<Option<S::Item>, S::Error> {
        let mut task = task.scoped();

        while self.remaining > 0 {
            match try_poll!(self.stream.poll(&mut task)) {
                Ok(Some(_)) => self.remaining -= 1,
                Ok(None) => return Poll::Ok(None),
                Err(e) => return Poll::Err(e),
            }
            task.ready();
        }

        self.stream.poll(&mut task)
    }

    fn schedule(&mut self, task: &mut Task) {
        self.stream.schedule(task)
    }
}
