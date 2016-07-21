use {Future, IntoFuture, Task, Poll};
use chain::Chain;

/// Future for the `and_then` combinator, chaining a computation onto the end of
/// another future which completes successfully.
///
/// This is created by this `Future::and_then` method.
pub struct AndThen<A, B, F> where A: Future, B: IntoFuture {
    state: Chain<A, B::Future, F>,
}

pub fn new<A, B, F>(future: A, f: F) -> AndThen<A, B, F>
    where A: Future,
          B: IntoFuture,
          F: Send + 'static,
{
    AndThen {
        state: Chain::new(future, f),
    }
}

impl<A, B, F> Future for AndThen<A, B, F>
    where A: Future,
          B: IntoFuture<Error=A::Error>,
          F: FnOnce(A::Item) -> B + Send + 'static,
{
    type Item = B::Item;
    type Error = B::Error;

    fn poll(&mut self, task: &mut Task) -> Poll<B::Item, B::Error> {
        self.state.poll(task, |result, f| {
            result.map(|e| {
                Err(f(e).into_future())
            })
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
