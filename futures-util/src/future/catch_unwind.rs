use futures_core::future::Future;
use futures_core::task::{Context, Poll};
use pin_project::{pin_project, unsafe_project};
use std::any::Any;
use std::panic::{catch_unwind, UnwindSafe, AssertUnwindSafe};
use std::pin::Pin;

/// Future for the [`catch_unwind`](super::FutureExt::catch_unwind) method.
#[unsafe_project(Unpin)]
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct CatchUnwind<Fut> where Fut: Future {
    #[pin]
    future: Fut,
}

impl<Fut> CatchUnwind<Fut> where Fut: Future + UnwindSafe {
    pub(super) fn new(future: Fut) -> CatchUnwind<Fut> {
        CatchUnwind { future }
    }
}

impl<Fut> Future for CatchUnwind<Fut>
    where Fut: Future + UnwindSafe,
{
    type Output = Result<Fut::Output, Box<dyn Any + Send>>;

    #[pin_project(self)]
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        catch_unwind(AssertUnwindSafe(|| self.future.poll(cx)))?.map(Ok)
    }
}
