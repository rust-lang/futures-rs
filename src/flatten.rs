use {Future, IntoFuture, Task, Poll};
use chain::Chain;

/// Future for the `flatten` combinator, flattening a future-of-a-future to get just
/// the result of the final future.
///
/// This is created by this `Future::flatten` method.
pub struct Flatten<A> where A: Future, A::Item: IntoFuture {
    state: Chain<A, <A::Item as IntoFuture>::Future, ()>,
}

pub fn new<A>(future: A) -> Flatten<A>
    where A: Future,
          A::Item: IntoFuture,
{
    Flatten {
        state: Chain::new(future, ()),
    }
}

impl<A> Future for Flatten<A>
    where A: Future,
          A::Item: IntoFuture,
          <<A as Future>::Item as IntoFuture>::Error: From<<A as Future>::Error>
{
    type Item = <<A as Future>::Item as IntoFuture>::Item;
    type Error = <<A as Future>::Item as IntoFuture>::Error;

    fn poll(&mut self, task: &mut Task) -> Poll<Self::Item, Self::Error> {
        self.state.poll(task, |a, ()| {
            let future = try!(a).into_future();
            Ok(Err(future))
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
