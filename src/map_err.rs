use {Future, Poll};

/// Future for the `map_err` combinator, changing the error type of a future.
///
/// This is created by this `Future::map_err` method.
pub struct MapErr<A, F> where A: Future {
    future: A,
    f: Option<F>,
}

pub fn new<A, F>(future: A, f: F) -> MapErr<A, F>
    where A: Future
{
    MapErr {
        future: future,
        f: Some(f),
    }
}

impl<U, A, F> Future for MapErr<A, F>
    where A: Future,
          F: FnOnce(A::Error) -> U + 'static,
          U: 'static,
{
    type Item = A::Item;
    type Error = U;

    fn poll(&mut self) -> Poll<A::Item, U> {
        let result = try_poll!(self.future.poll());
        result.map_err(self.f.take().expect("cannot poll MapErr twice")).into()
    }
}
