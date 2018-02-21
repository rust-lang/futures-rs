use futures_core::{Async, Future, IntoFuture, Poll, Stream};
use futures_core::task;

/// A stream combinator which executes a unit closure over each item on a
/// stream.
///
/// This structure is returned by the `Stream::for_each` method.
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct ForEach<S, F, U> where U: IntoFuture {
    stream: Option<S>,
    f: F,
    fut: Option<U::Future>,
}

pub fn new<S, F, U>(s: S, f: F) -> ForEach<S, F, U>
    where S: Stream,
          F: FnMut(S::Item) -> U,
          U: IntoFuture<Item = (), Error = S::Error>,
{
    ForEach {
        stream: Some(s),
        f: f,
        fut: None,
    }
}

impl<S, F, U> Future for ForEach<S, F, U>
    where S: Stream,
          F: FnMut(S::Item) -> U,
          U: IntoFuture<Item= (), Error = S::Error>,
{
    type Item = S;
    type Error = S::Error;

    fn poll(&mut self, cx: &mut task::Context) -> Poll<S, S::Error> {
        loop {
            if let Some(mut fut) = self.fut.take() {
                if fut.poll(cx)?.is_not_ready() {
                    self.fut = Some(fut);
                    return Ok(Async::Pending);
                }
            }

            match self.stream {
                Some(ref mut stream) => {
                    match try_ready!(stream.poll_next(cx)) {
                        Some(e) => self.fut = Some((self.f)(e).into_future()),
                        None => break,
                    }
                }
                None => panic!("poll after a ForEach was done"),
            }
        }
        Ok(Async::Ready(self.stream.take().unwrap()))
    }
}
