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

impl<S, F> Future for ForEach<S, F>
    where S: Stream,
          F: FnMut(S::Item) -> Result<(), S::Error>,
{
    type Item = S;
    type Error = S::Error;

    fn poll(&mut self) -> Poll<S, S::Error> {
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

        Ok(Async::Ready(self.stream.take().unwrap()))
    }
}
