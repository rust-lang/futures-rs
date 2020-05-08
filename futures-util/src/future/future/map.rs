use core::pin::Pin;
use futures_core::future::{FusedFuture, Future};
use futures_core::task::{Context, Poll};
use pin_project::{pin_project, project, project_replace};

use crate::fns::FnOnce1;

/// Internal Map future
#[pin_project(Replace)]
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub enum Map<Fut, F> {
    Incomplete {
        #[pin]
        future: Fut,
        f: F,
    },
    Complete,
}

impl<Fut, F> Map<Fut, F> {
    /// Creates a new Map.
    pub(crate) fn new(future: Fut, f: F) -> Map<Fut, F> {
        Map::Incomplete { future, f }
    }
}

impl<Fut, F, T> FusedFuture for Map<Fut, F>
    where Fut: Future,
          F: FnOnce1<Fut::Output, Output=T>,
{
    fn is_terminated(&self) -> bool {
        match self {
            Map::Incomplete { .. } => false,
            Map::Complete => true,
        }
    }
}

impl<Fut, F, T> Future for Map<Fut, F>
    where Fut: Future,
          F: FnOnce1<Fut::Output, Output=T>,
{
    type Output = T;

    #[project]
    #[project_replace]
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<T> {
        #[project]
        match self.as_mut().project() {
            Map::Incomplete { future, .. } => {
                let output = ready!(future.poll(cx));
                #[project_replace]
                match self.project_replace(Map::Complete) {
                    Map::Incomplete { f, .. } => Poll::Ready(f.call_once(output)),
                    Map::Complete => unreachable!(),
                }
            },
            Map::Complete => panic!("Map must not be polled after it returned `Poll::Ready`"),
        }
    }
}
