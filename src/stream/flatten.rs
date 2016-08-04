use {Task, Poll};
use stream::Stream;

/// A combinator used to flatten a stream-of-streams into one long stream of
/// elements.
///
/// This combinator is created by the `Stream::flatten` method.
pub struct Flatten<S>
    where S: Stream,
{
    stream: S,
    next: Option<S::Item>,
}

pub fn new<S>(s: S) -> Flatten<S>
    where S: Stream,
          S::Item: Stream,
          <S::Item as Stream>::Error: From<S::Error>,
{
    Flatten {
        stream: s,
        next: None,
    }
}

impl<S> Stream for Flatten<S>
    where S: Stream,
          S::Item: Stream,
          <S::Item as Stream>::Error: From<S::Error>,
{
    type Item = <S::Item as Stream>::Item;
    type Error = <S::Item as Stream>::Error;

    fn poll(&mut self, task: &mut Task)
            -> Poll<Option<Self::Item>, Self::Error> {
        loop {
            if self.next.is_none() {
                match try_poll!(self.stream.poll(task)) {
                    Ok(Some(e)) => self.next = Some(e),
                    Ok(None) => return Poll::Ok(None),
                    Err(e) => return Poll::Err(From::from(e)),
                }
            }
            assert!(self.next.is_some());
            match self.next.as_mut().unwrap().poll(task) {
                Poll::Ok(None) => self.next = None,
                other => return other,
            }
        }
    }

    fn schedule(&mut self, task: &mut Task) {
        match self.next {
            Some(ref mut s) => s.schedule(task),
            None => self.stream.schedule(task),
        }
    }
}
