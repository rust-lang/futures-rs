use core::marker::Unpin;
use core::mem::PinMut;
use futures_core::future::Future;
use futures_core::task::{self, Poll, Executor};

/// Future for the `with_executor` combinator, assigning an executor
/// to be used when spawning other futures.
///
/// This is created by the `Future::with_executor` method.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct WithExecutor<Fut, E> where Fut: Future, E: Executor {
    executor: E,
    future: Fut
}

impl<Fut: Future, E: Executor> WithExecutor<Fut, E> {
    pub(super) fn new(future: Fut, executor: E) -> WithExecutor<Fut, E> {
        WithExecutor { executor, future }
    }
}

impl<Fut: Future + Unpin, E: Executor> Unpin for WithExecutor<Fut, E> {}

impl<Fut, E> Future for WithExecutor<Fut, E>
    where Fut: Future,
          E: Executor,
{
    type Output = Fut::Output;

    fn poll(self: PinMut<Self>, cx: &mut task::Context) -> Poll<Fut::Output> {
        let this = unsafe { PinMut::get_mut_unchecked(self) };
        let fut = unsafe { PinMut::new_unchecked(&mut this.future) };
        let exec = &mut this.executor;
        fut.poll(&mut cx.with_executor(exec))
    }
}
