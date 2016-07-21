use {Task, Poll};
use stream::Stream;

/// A stream combinator which will change the type of a stream from one
/// type to another.
///
/// This is produced by the `Stream::map` method.
pub struct Map<S, F> {
    stream: S,
    f: F,
}

pub fn new<S, F, U>(s: S, f: F) -> Map<S, F>
    where S: Stream,
          F: FnMut(S::Item) -> U + Send + 'static,
          U: Send + 'static,
{
    Map {
        stream: s,
        f: f,
    }
}

impl<S, F, U> Stream for Map<S, F>
    where S: Stream,
          F: FnMut(S::Item) -> U + Send + 'static,
          U: Send + 'static,
{
    type Item = U;
    type Error = S::Error;

    fn poll(&mut self, task: &mut Task) -> Poll<Option<U>, S::Error> {
        self.stream.poll(task).map(|option| option.map(&mut self.f))
    }

    fn schedule(&mut self, task: &mut Task) {
        self.stream.schedule(task)
    }
}
