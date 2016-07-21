use {Task, Poll};
use stream::Stream;

/// A stream combinator which will change the error type of a stream from one
/// type to another.
///
/// This is produced by the `Stream::map_err` method.
pub struct MapErr<S, F> {
    stream: S,
    f: F,
}

pub fn new<S, F, U>(s: S, f: F) -> MapErr<S, F>
    where S: Stream,
          F: FnMut(S::Error) -> U + Send + 'static,
          U: Send + 'static,
{
    MapErr {
        stream: s,
        f: f,
    }
}

impl<S, F, U> Stream for MapErr<S, F>
    where S: Stream,
          F: FnMut(S::Error) -> U + Send + 'static,
          U: Send + 'static,
{
    type Item = S::Item;
    type Error = U;

    fn poll(&mut self, task: &mut Task) -> Poll<Option<S::Item>, U> {
        self.stream.poll(task).map_err(&mut self.f)
    }

    fn schedule(&mut self, task: &mut Task) {
        self.stream.schedule(task)
    }
}
