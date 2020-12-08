use core::pin::Pin;
use futures_core::future::{FusedFuture, Future};
use futures_core::ready;
use futures_core::task::{Context, Poll};
use pin_project_lite::pin_project;

use crate::fns::FnOnce1;

pin_project! {
    /// Internal Map future
    #[project = MapProj]
    #[derive(Debug)]
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    pub enum Map<Fut, F> {
        Incomplete {
            #[pin]
            future: Fut,
            f: Option<F>,
        },
        Complete,
    }
}

impl<Fut, F> Map<Fut, F> {
    /// Creates a new Map.
    pub(crate) fn new(future: Fut, f: F) -> Self {
        Self::Incomplete { future, f: Some(f) }
    }
}

impl<Fut, F, T> FusedFuture for Map<Fut, F>
    where Fut: Future,
          F: FnOnce1<Fut::Output, Output=T>,
{
    fn is_terminated(&self) -> bool {
        match self {
            Self::Incomplete { .. } => false,
            Self::Complete => true,
        }
    }
}

impl<Fut, F, T> Future for Map<Fut, F>
    where Fut: Future,
          F: FnOnce1<Fut::Output, Output=T>,
{
    type Output = T;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<T> {
        match self.as_mut().project() {
            MapProj::Incomplete { future, f } => {
                let output = ready!(future.poll(cx));
                let f = f.take().unwrap();
                self.set(Self::Complete);
                Poll::Ready(f.call_once(output))
            },
            MapProj::Complete => panic!("Map must not be polled after it returned `Poll::Ready`"),
        }
    }
}
