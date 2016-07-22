use {Task, Poll};
use stream::Stream;

/// A stream combinator which returns a maximum number of elements.
///
/// This structure is produced by the `Stream::take` method.
pub struct Take<S> {
    stream: S,
    remaining: u64,
}

pub fn new<S>(s: S, amt: u64) -> Take<S>
    where S: Stream,
{
    Take {
        stream: s,
        remaining: amt,
    }
}

impl<S> Stream for Take<S>
    where S: Stream,
{
    type Item = S::Item;
    type Error = S::Error;

    fn poll(&mut self, task: &mut Task) -> Poll<Option<S::Item>, S::Error> {
        if self.remaining == 0 {
            Poll::Ok(None)
        } else {
            match self.stream.poll(task) {
                Poll::Ok(Some(e)) => {
                    self.remaining -= 1;
                    Poll::Ok(Some(e))
                }
                other => other,
            }
        }
    }

    fn schedule(&mut self, task: &mut Task) {
        if self.remaining == 0 {
            task.notify()
        } else {
            self.stream.schedule(task)
        }
    }
}
