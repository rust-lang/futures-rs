use {IntoFuture, Future, Poll, Async};
use stream::Stream;

/// A stream combinator which chains a computation onto errors produced by a
/// stream.
///
/// This structure is produced by the `Stream::or_else` method.
pub struct OrElse<S, F, U>
    where U: IntoFuture,
{
    stream: S,
    future: Option<U::Future>,
    f: F,
}

pub fn new<S, F, U>(s: S, f: F) -> OrElse<S, F, U>
    where S: Stream,
          F: FnMut(S::Error) -> U,
          U: IntoFuture<Item=S::Item>,
{
    OrElse {
        stream: s,
        future: None,
        f: f,
    }
}

impl<S, F, U> Stream for OrElse<S, F, U>
    where S: Stream,
          F: FnMut(S::Error) -> U,
          U: IntoFuture<Item=S::Item>,
{
    type Item = S::Item;
    type Error = U::Error;

    fn poll(&mut self) -> Poll<Option<S::Item>, U::Error> {
        if self.future.is_none() {
            let item = match self.stream.poll() {
                Ok(Async::Ready(e)) => return Ok(Async::Ready(e)),
                Ok(Async::NotReady) => return Ok(Async::NotReady),
                Err(e) => e,
            };
            self.future = Some((self.f)(item).into_future());
        }
        assert!(self.future.is_some());
        match self.future.as_mut().unwrap().poll() {
            Ok(Async::Ready(e)) => {
                self.future = None;
                Ok(Async::Ready(Some(e)))
            }
            Err(e) => {
                self.future = None;
                Err(e)
            }
            Ok(Async::NotReady) => Ok(Async::NotReady)
        }
    }
}
