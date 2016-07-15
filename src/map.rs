use std::sync::Arc;

use {Future, Wake, Tokens};
use util::Collapsed;

/// Future for the `map` combinator, changing the type of a future.
///
/// This is created by this `Future::map` method.
pub struct Map<A, F> where A: Future {
    future: Collapsed<A>,
    f: Option<F>,
}

pub fn new<A, F>(future: A, f: F) -> Map<A, F>
    where A: Future,
{
    Map {
        future: Collapsed::Start(future),
        f: Some(f),
    }
}

impl<U, A, F> Future for Map<A, F>
    where A: Future,
          F: FnOnce(A::Item) -> U + Send + 'static,
          U: Send + 'static,
{
    type Item = U;
    type Error = A::Error;

    fn poll(&mut self, tokens: &Tokens) -> Option<Result<U, A::Error>> {
        self.future.poll(tokens).map(|result| {
            result.map(self.f.take().expect("cannot poll Map twice"))
        })
    }

    fn schedule(&mut self, wake: &Arc<Wake>) {
        self.future.schedule(wake)
    }

    fn tailcall(&mut self)
                -> Option<Box<Future<Item=Self::Item, Error=Self::Error>>> {
        self.future.collapse();
        None
    }
}
