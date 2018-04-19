use core::mem::Pin;

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
    type Output = F::Output;

    fn poll(mut self: Pin<Self>, cx: &mut task::Context) -> Poll<F::Output> {
        let this = unsafe { Pin::get_mut(&mut self) };
        let fut = unsafe { Pin::new_unchecked(&mut this.future) };
        let exec = &mut this.executor;
        fut.poll(&mut cx.with_executor(exec))
    }
}
