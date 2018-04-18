use futures_core::{Async, Future, IntoFuture, Poll, Stream};
use futures_core::task;

use super::futures_unordered::FuturesUnordered;

/// A stream combinator which executes a unit closure over each item on a
/// stream concurrently.
///
/// This structure is returned by the `Stream::for_each` method.
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct ForEachConcurrent<S, U, F> where U: IntoFuture {
    stream: Option<S>,
    stream_done: bool,
    f: F,
    futures: FuturesUnordered<U::Future>,
}

pub fn new<S, U, F>(s: S, f: F) -> ForEachConcurrent<S, U, F>
    where S: Stream,
          F: FnMut(S::Item) -> U,
          U: IntoFuture<Item = (), Error = S::Error>,
{
    ForEachConcurrent {
        stream: Some(s),
        stream_done: false,
        f: f,
        futures: FuturesUnordered::new(),
    }
}

impl<S, U, F> Future for ForEachConcurrent<S, U, F>
    where S: Stream,
          F: FnMut(S::Item) -> U,
          U: IntoFuture<Item= (), Error = S::Error>,
{
    type Item = S;
    type Error = S::Error;

    fn poll(&mut self, cx: &mut task::Context) -> Poll<S, S::Error> {
        loop {
            let mut made_progress_this_iter = false;

            // Try and pull an item off of the stream
            if !self.stream_done {
                // `unwrap` is valid because the stream is only taken after `stream_done` is set
                match self.stream.as_mut().unwrap().poll_next(cx)? {
                    Async::Ready(Some(x)) => {
                        self.futures.push((self.f)(x).into_future());
                        made_progress_this_iter = true;
                    }
                    // The stream completed, so it shouldn't be polled
                    // anymore.
                    Async::Ready(None) => self.stream_done = true,
                    Async::Pending => {},
                }
            }

            match self.futures.poll_next(cx)? {
                Async::Ready(Some(())) => made_progress_this_iter = true,
                Async::Ready(None) if self.stream_done => {
                    // We've processed all of self.futures and self.stream,
                    // so return self.stream
                    return Ok(Async::Ready(self.stream.take().expect(
                        "polled for_each_concurrent after completion"
                    )));
                }
                Async::Ready(None)
                | Async::Pending => {}
            }

            if !made_progress_this_iter {
                return Ok(Async::Pending);
            }
        }
    }
}
