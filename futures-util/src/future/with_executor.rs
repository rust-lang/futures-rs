use futures_core::{Future, Poll};
use futures_core::task;
use futures_core::executor::Executor;

/// Future for the `with_executor` combinator, assigning an executor
/// to be used when spawning other futures.
///
/// This is created by the `Future::with_executor` method.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct WithExecutor<F, E> where F: Future, E: Executor {
    executor: E,
    future: F
}

pub fn new<F, E>(future: F, executor: E) -> WithExecutor<F, E>
    where F: Future,
          E: Executor,
{
    WithExecutor { executor, future }
}

impl<F, E> Future for WithExecutor<F, E>
    where F: Future,
          E: Executor,
{
    type Item = F::Item;
    type Error = F::Error;

    fn poll(&mut self, cx: &mut task::Context) -> Poll<F::Item, F::Error> {
        self.future.poll(&mut cx.with_executor(&mut self.executor))
    }
}
