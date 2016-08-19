use Poll;
use stream::{Stream, Fuse};

/// A `Stream` that implements a `peek` method.
///
/// The `peek` method can be used to retrieve a reference
/// to the next `Stream::Item` if available. A subsequent
/// call to `poll` will return the owned item.
pub struct Peekable<S: Stream> {
    stream: Fuse<S>,
    peeked: Option<S::Item>,
}


pub fn new<S: Stream>(stream: S) -> Peekable<S> {
    Peekable {
        stream: stream.fuse(),
        peeked: None
    }
}


impl<S: Stream> Stream for Peekable<S> {
    type Item = S::Item;
    type Error = S::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        if let Some(item) = self.peeked.take() {
            return Poll::Ok(Some(item));
        }
        self.stream.poll()
    }
}


impl<S: Stream> Peekable<S> {
    /// Peek retrieves a reference to the next item in the stream.
    ///
    /// This method polls the underlying stream and return either a reference
    /// to the next item if the stream is ready or passes through any errors.
    pub fn peek(&mut self) -> Poll<Option<&S::Item>, S::Error> {
        if self.peeked.is_some() {
            return Poll::Ok(self.peeked.as_ref());
        }
        match self.poll() {
            Poll::NotReady => Poll::NotReady,
            Poll::Err(e) => Poll::Err(e),
            Poll::Ok(None) => Poll::Ok(None),
            Poll::Ok(Some(item)) => {
                self.peeked = Some(item);
                Poll::Ok(self.peeked.as_ref())
            }
        }
    }
}
