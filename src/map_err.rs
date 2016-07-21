use {Future, Task, Poll};
use util::Collapsed;

/// Future for the `map_err` combinator, changing the error type of a future.
///
/// This is created by this `Future::map_err` method.
pub struct MapErr<A, F> where A: Future {
    future: Collapsed<A>,
    f: Option<F>,
}

pub fn new<A, F>(future: A, f: F) -> MapErr<A, F>
    where A: Future
{
    MapErr {
        future: Collapsed::Start(future),
        f: Some(f),
    }
}

impl<U, A, F> Future for MapErr<A, F>
    where A: Future,
          F: FnOnce(A::Error) -> U + Send + 'static,
          U: Send + 'static,
{
    type Item = A::Item;
    type Error = U;

    fn poll(&mut self, task: &mut Task) -> Poll<A::Item, U> {
        let result = try_poll!(self.future.poll(task));
        result.map_err(self.f.take().expect("cannot poll MapErr twice")).into()
    }

    fn schedule(&mut self, task: &mut Task) {
        self.future.schedule(task)
    }

    fn tailcall(&mut self)
                -> Option<Box<Future<Item=Self::Item, Error=Self::Error>>> {
        self.future.collapse();
        None
    }
}
