use std::sync::Arc;

use {Future, PollResult, Wake};
use util;

/// Future for the `map` combinator, changing the type of a future.
///
/// This is created by this `Future::map` method.
pub struct Map<A, F> {
    future: A,
    f: Option<F>,
}

pub fn new<A, F>(future: A, f: F) -> Map<A, F> {
    Map {
        future: future,
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

    fn poll(&mut self) -> Option<PollResult<U, A::Error>> {
        let result = match self.future.poll() {
            Some(result) => result,
            None => return None,
        };
        let callback = util::opt2poll(self.f.take());
        Some(result.and_then(|e| {
            callback.map(|f| f(e))
        }))
    }

    fn schedule(&mut self, wake: Arc<Wake>) {
        self.future.schedule(wake)
    }
}
