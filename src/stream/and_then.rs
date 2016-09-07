use {IntoFuture, Future, Poll, Async};
use stream::Stream;

/// A stream combinator which chains a computation onto values produced by a
/// stream.
///
/// This structure is produced by the `Stream::and_then` method.
#[must_use = "streams do nothing unless polled"]
pub struct AndThen<S, F, U>
    where U: IntoFuture,
{
    stream: S,
    future: Option<U::Future>,
    f: F,
}

pub fn new<S, F, U>(s: S, f: F) -> AndThen<S, F, U>
    where S: Stream,
          F: FnMut(S::Item) -> U,
          U: IntoFuture<Error=S::Error>,
{
    AndThen {
        stream: s,
        future: None,
        f: f,
    }
}

impl<S, F, U> Stream for AndThen<S, F, U>
    where S: Stream,
          F: FnMut(S::Item) -> U,
          U: IntoFuture<Error=S::Error>,
{
    type Item = U::Item;
    type Error = S::Error;

    fn poll(&mut self) -> Poll<Option<U::Item>, S::Error> {
        if self.future.is_none() {
            let item = match try_ready!(self.stream.poll()) {
                None => return Ok(Async::Ready(None)),
                Some(e) => e,
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
