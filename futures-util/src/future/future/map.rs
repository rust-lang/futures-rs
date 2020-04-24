use core::pin::Pin;
use core::ptr;
use futures_core::future::{FusedFuture, Future};
use futures_core::task::{Context, Poll};
use pin_project::{pin_project, project};

use crate::fns::FnOnce1;

/// Internal Map future
#[pin_project]
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

// Helper type to mark a `Map` as complete without running its destructor.
struct UnsafeMarkAsComplete<Fut, F>(*mut Map<Fut, F>);

impl<Fut, F> Drop for UnsafeMarkAsComplete<Fut, F> {
    fn drop(&mut self) {
        unsafe {
            ptr::write(self.0, Map::Complete);
        }
    }
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
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<T> {
        unsafe {
            // Store this pointer for later...
            let self_ptr: *mut Self = self.as_mut().get_unchecked_mut();
            
            match &mut *self_ptr {
                Map::Incomplete { future, f } => {
                    let mut future = Pin::new_unchecked(future);
                    let output = match future.as_mut().poll(cx) {
                        Poll::Ready(x) => x,
                        Poll::Pending => return Poll::Pending,
                    };
    
                    // Here be dragons
                    let f = ptr::read(f);
                    {
                        // The ordering here is important, the call to `drop_in_place` must be
                        // last as it may panic. Other lines must not panic.
                        let _cleanup = UnsafeMarkAsComplete(self_ptr);
                        ptr::drop_in_place(future.get_unchecked_mut());
                    };

                    // Phew, everything is back to normal, and we should be in the
                    // `Complete` state!
                    Poll::Ready(f.call_once(output))
                },
                Map::Complete => panic!("Map must not be polled after it returned `Poll::Ready`"),
            }
        }
    }
}
