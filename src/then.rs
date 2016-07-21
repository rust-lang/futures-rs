use {Future, IntoFuture, Task, Poll};
use chain::Chain;

/// Future for the `then` combinator, chaining computations on the end of
/// another future regardless of its outcome.
///
/// This is created by this `Future::then` method.
pub struct Then<A, B, F> where A: Future, B: IntoFuture {
    state: Chain<A, B::Future, F>,
}

pub fn new<A, B, F>(future: A, f: F) -> Then<A, B, F>
    where A: Future,
          B: IntoFuture,
          F: Send + 'static,
{
    Then {
        state: Chain::new(future, f),
    }
}

impl<A, B, F> Future for Then<A, B, F>
    where A: Future,
          B: IntoFuture,
          F: FnOnce(Result<A::Item, A::Error>) -> B + Send + 'static,
{
    type Item = B::Item;
    type Error = B::Error;

    fn poll(&mut self, task: &mut Task) -> Poll<B::Item, B::Error> {
        self.state.poll(task, |a, f| {
            Ok(Err(f(a).into_future()))
        })
    }

    fn schedule(&mut self, task: &mut Task) {
        self.state.schedule(task)
    }

    fn tailcall(&mut self)
                -> Option<Box<Future<Item=Self::Item, Error=Self::Error>>> {
        self.state.tailcall()
    }
}
