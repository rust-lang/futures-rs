use Poll;
use stream::{Stream, Fuse};

/// An adapter for merging the output of two streams.
///
/// The merged stream produces items from one or both of the underlying
/// streams as they become available. Errors, however, are not merged: you
/// get at most one error at a time.
pub struct Zip<S1: Stream, S2: Stream> {
    stream1: Fuse<S1>,
    stream2: Fuse<S2>,
    queued1: Option<S1::Item>,
    queued2: Option<S2::Item>,
}

pub fn new<S1, S2>(stream1: S1, stream2: S2) -> Zip<S1, S2>
    where S1: Stream, S2: Stream<Error = S1::Error>
{
    Zip {
        stream1: stream1.fuse(),
        stream2: stream2.fuse(),
        queued1: None,
        queued2: None,
    }
}

impl<S1, S2> Stream for Zip<S1, S2>
    where S1: Stream, S2: Stream<Error = S1::Error>
{
    type Item = (S1::Item, S2::Item);
    type Error = S1::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        if self.queued1.is_none() {
            match self.stream1.poll() {
                Poll::Err(e) => return Poll::Err(e),
                Poll::NotReady => {}
                Poll::Ok(Some(item1)) => self.queued1 = Some(item1),
                Poll::Ok(None) => {}
            }
        }
        if self.queued2.is_none() {
            match self.stream2.poll() {
                Poll::Err(e) => return Poll::Err(e),
                Poll::NotReady => {}
                Poll::Ok(Some(item2)) => self.queued2 = Some(item2),
                Poll::Ok(None) => {}
            }
        }

        if self.queued1.is_some() && self.queued2.is_some() {
            let pair = (self.queued1.take().unwrap(),
                        self.queued2.take().unwrap());
            Poll::Ok(Some(pair))
        } else if self.stream1.is_done() || self.stream2.is_done() {
            Poll::Ok(None)
        } else {
            Poll::NotReady
        }
    }
}
