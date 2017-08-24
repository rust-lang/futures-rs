use {Stream, Poll, Async};

/// Do something with the items of a stream, passing it on.
///
/// This is created by the `Stream::inspect` method.
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct Inspect<S, F> where S: Stream {
    stream: S,
    inspect: F,
}

pub fn new<S, F>(stream: S, f: F) -> Inspect<S, F>
    where S: Stream,
          F: FnMut(&S::Item) -> (),
{
    Inspect {
        stream: stream,
        inspect: f,
    }
}

impl<S, F> Stream for Inspect<S, F>
    where S: Stream,
          F: FnMut(&S::Item),
{
    type Item = S::Item;
    type Error = S::Error;

    fn poll(&mut self) -> Poll<Option<S::Item>, S::Error> {
        match try_ready!(self.stream.poll()) {
            Some(e) => {
                (self.inspect)(&e);
                Ok(Async::Ready(Some(e)))
            }
            None => Ok(Async::Ready(None)),
        }
    }
}
