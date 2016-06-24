use std::sync::Arc;

use {Future, PollResult, Wake, Tokens};
use util::{self, Collapsed};

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

    fn poll(&mut self, tokens: &Tokens) -> Option<PollResult<A::Item, U>> {
        let result = match self.future.poll(tokens) {
            Some(result) => result,
            None => return None,
        };
        let f = util::opt2poll(self.f.take());
        Some(f.and_then(|f| {
            result.map_err(|e| e.map(f))
        }))
    }

    fn schedule(&mut self, wake: Arc<Wake>) -> Tokens {
        self.future.schedule(wake)
    }

    fn tailcall(&mut self)
                -> Option<Box<Future<Item=Self::Item, Error=Self::Error>>> {
        self.future.collapse();
        None
    }
}
