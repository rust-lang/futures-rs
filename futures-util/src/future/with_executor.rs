use core::marker::Unpin;
use core::mem::PinMut;
use futures_core::future::Future;
use futures_core::task::{Context, Poll, Executor};

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

impl<F: Future + Unpin, E: Executor> Unpin for WithExecutor<F, E> {}

impl<F, E> Future for WithExecutor<F, E>
    where F: Future,
          E: Executor,
{
    type Output = F::Output;

    fn poll(self: PinMut<Self>, cx: &mut Context) -> Poll<F::Output> {
        let this = unsafe { PinMut::get_mut_unchecked(self) };
        let fut = unsafe { PinMut::new_unchecked(&mut this.future) };
        let exec = &mut this.executor;
        fut.poll(&mut cx.with_executor(exec))
    }
}
