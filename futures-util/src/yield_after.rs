use core::num::NonZeroU32;
use futures_core::iteration::Limit;

// Default for repetition limits on eager polling loops, to prevent
// stream-consuming combinators like ForEach from starving other tasks.
pub(crate) const DEFAULT_YIELD_AFTER_LIMIT: Limit = Limit::new(
    unsafe { NonZeroU32::new_unchecked(100) }
);

macro_rules! method_yield_after_every {
    ($(#[$doc:meta])*) => {
        $(#[$doc])*
        pub fn yield_after_every(mut self, iterations: u32) -> Self {
            let v = core::num::NonZeroU32::new(iterations)
                .expect("iteration limit can't be 0");
            self.yield_after = futures_core::iteration::Limit::new(v);
            self
        }
    }
}

macro_rules! future_method_yield_after_every {
    () => {
        future_method_yield_after_every! {
            #[doc = "the underlying stream"]
            #[doc = "the stream consecutively yields items,"]
        }
    };
    (#[$pollee:meta] #[$why_busy:meta]) => {
        method_yield_after_every! {
            /// Changes the maximum number of iterations before `poll` yields.
            ///
            /// The implementation of [`poll`] on this future
            /** polls */#[$pollee]/** in a loop. */
            /// To prevent blocking in the call to `poll` for too long while
            #[$why_busy]
            /// the number of iterations is capped to an internal limit,
            /// after which the `poll` function wakes up the task
            /// and returns [`Pending`]. The `yield_after_every` combinator
            /// can be used to tune the iteration limit, returning a future
            /// with the limit updated to the provided value.
            ///
            /// [`poll`]: core::future::Future::poll
            /// [`Pending`]: core::task::poll::Poll::Pending
            ///
            /// # Panics
            ///
            /// If called with 0 as the number of iterations, this method panics.
        }
    };
}

macro_rules! try_future_method_yield_after_every {
    () => {
        future_method_yield_after_every! {
            #[doc = "the underlying stream"]
            #[doc = "the stream consecutively yields `Ok` items,"]
        }    
    }
}

macro_rules! stream_method_yield_after_every {
    () => {
        stream_method_yield_after_every! {
            #[doc = "the underlying stream"]
            #[doc = "the stream consecutively yields items,"]
        }
    };
    (#[$pollee:meta] #[$why_busy:meta]) => {
        method_yield_after_every! {
            /// Changes the maximum number of iterations before `poll_next` yields.
            ///
            /// The implementation of [`poll_next`] on this stream
            /** polls */#[$pollee]/** in a loop. */
            /// To prevent blocking in the call to `poll_next` for too long while
            #[$why_busy]
            /// the number of iterations is capped to an internal limit,
            /// after which the `poll_next` function wakes up the task
            /// and returns [`Pending`]. The `yield_after_every` combinator
            /// can be used to tune the iteration limit, returning a stream
            /// with the limit updated to the provided value.
            ///
            /// [`poll_next`]: crate::stream::Stream::poll_next
            /// [`Pending`]: core::task::poll::Poll::Pending
            ///
            /// # Panics
            ///
            /// If called with 0 as the number of iterations, this method panics.
        }
    };
}
