use {Async, Future, Poll};
use stream::Stream;

/// A stream combinator which executes a unit closure over each item on a
/// stream.
///
/// This structure is returned by the `Stream::for_each` method.
#[must_use = "streams do nothing unless polled"]
pub struct ForEach<S, F> {
    stream: Option<S>,
    f: F,
}

pub fn new<S, F>(s: S, f: F) -> ForEach<S, F>
    where S: Stream,
          F: FnMut(S::Item) -> Result<(), S::Error>,
{
    ForEach {
        stream: Some(s),
        f: f,
    }
}

/// Wrapper around a stream that has already reached its end.
pub struct Drained<S>(pub S);

impl<S, F> Future for ForEach<S, F>
    where S: Stream,
          F: FnMut(S::Item) -> Result<(), S::Error>,
{
    type Item = Drained<S>;
    type Error = S::Error;

    fn poll(&mut self) -> Poll<Drained<S>, S::Error> {
        match self.stream {
            Some(ref mut s) => {
                loop {
                    let r = try_ready!(s.poll());
                    match r {
                        Some(e) => try!((self.f)(e)),
                        None => break,
                    }
                }

            }
            None => panic!("poll a ForEach after it's done"),
        }

        Ok(Async::Ready(Drained(self.stream.take().unwrap())))
    }
}
