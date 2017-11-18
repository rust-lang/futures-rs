use {Async, Future, IntoFuture, Poll};
use stream::{Stream, FuturesUnordered};


/// A stream combinator which executes a unit closure over each item on a
/// stream.
///
/// This structure is returned by the `Stream::any` method.
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct Any<S, F, U> where U: IntoFuture {
    stream: S,
    eof: bool,
    f: F,
    futures: FuturesUnordered<U::Future>,
}

pub fn new<S, F, U>(s: S, f: F) -> Any<S, F, U>
    where S: Stream,
          F: FnMut(S::Item) -> U,
          U: IntoFuture,
{
    Any {
        stream: s,
        eof: false,
        f: f,
        futures: FuturesUnordered::new(),
    }
}

impl<S, F, U> Future for Any<S, F, U>
    where S: Stream,
          F: FnMut(S::Item) -> U,
          U: IntoFuture,
{
    type Item = ();
    type Error = S::Error;

    fn poll(&mut self) -> Poll<(), S::Error> {
        while !self.eof {
            match self.stream.poll()? {
                Async::Ready(Some(d)) => {
                    let f = (self.f)(d).into_future();
                    self.futures.push(f);
                }
                Async::Ready(None) => self.eof = true,
                Async::NotReady => break,
            }
        }

        loop {
            match self.futures.poll() {
                Err(_) |
                Ok(Async::Ready(Some(_)))  => {}
                Ok(Async::Ready(None)) => {
                    if self.eof {
                        return Ok(Async::Ready(()));
                    } else {
                        return Ok(Async::NotReady);
                    }
                }
                Ok(Async::NotReady) => return Ok(Async::NotReady),
            }
        }
    }
}
