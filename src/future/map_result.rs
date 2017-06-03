use {Future, Poll, Async};

/// Future for the `map_result` combinator, changing the type of a future.
///
/// This is created by the `Future::map_result` method.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct MapResult<A, F> where A: Future {
    future: A,
    f: Option<F>,
}

pub fn new<A, F>(future: A, f: F) -> MapResult<A, F>
    where A: Future,
{
    MapResult {
        future: future,
        f: Some(f),
    }
}

impl<U, A, F> Future for MapResult<A, F>
    where A: Future,
          F: FnOnce(A::Item) -> Result<U, A::Error>,
{
    type Item = U;
    type Error = A::Error;

    fn poll(&mut self) -> Poll<U, A::Error> {
        let e = match self.future.poll() {
            Ok(Async::NotReady) => return Ok(Async::NotReady),
            Ok(Async::Ready(e)) => Ok(e),
            Err(e) => Err(e),
        };
        e.and_then(self.f.take().expect("cannot poll Map twice"))
         .map(Async::Ready)
    }
}
