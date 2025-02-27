use core::num::NonZeroUsize;
use core::pin::Pin;
use core::task::Context;
use core::task::Poll;

use futures_core::ready;
use futures_core::FusedStream;
use futures_core::Stream;
use pin_project_lite::pin_project;

pin_project! {
    /// Stream for the [`merge_round_robin`](crate::Streamies::merge_round_robin) method.
    ///  #[derive(Debug)]
    #[must_use = "streams do nothing unless polled"]
    pub struct MergeRoundRobin<St1, St2> {
        #[pin]
        first: Option<St1>,
        #[pin]
        second: Option<St2>,

        first_nb_ele: NonZeroUsize,
        second_nb_ele: NonZeroUsize,

        first_count: usize,
        second_count: usize
    }
}

impl<St1, St2> MergeRoundRobin<St1, St2>
where
    St1: Stream,
    St2: Stream<Item = St1::Item>,
{
    pub(super) fn new(
        stream1: St1,
        stream2: St2,
        first_nb_ele: usize,
        second_nb_ele: usize,
    ) -> Self {
        Self {
            first: Some(stream1),
            second: Some(stream2),
            first_nb_ele: NonZeroUsize::new(first_nb_ele).expect(
                "Couldn't convert `first_nb_ele` to `NonZeroUsize`. The value must no be 0",
            ),
            second_nb_ele: NonZeroUsize::new(second_nb_ele).expect(
                "Couldn't convert `second_nb_ele` to `NonZeroUsize`. The value must no be 0",
            ),
            first_count: 0,
            second_count: 0,
        }
    }
}

impl<St1, St2> FusedStream for MergeRoundRobin<St1, St2>
where
    St1: FusedStream,
    St2: FusedStream<Item = St1::Item>,
{
    fn is_terminated(&self) -> bool {
        self.first.as_ref().is_none_or(|s| s.is_terminated())
            && self.second.as_ref().is_none_or(|s| s.is_terminated())
    }
}

impl<St1, St2> Stream for MergeRoundRobin<St1, St2>
where
    St1: Stream,
    St2: Stream<Item = St1::Item>,
{
    type Item = St1::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();
        // Check if second stream has finished it's turn
        if this.second_count == &mut this.second_nb_ele.get() {
            // It finished. Let's reset the turns
            *this.first_count = 0;
            *this.second_count = 0;
        }

        // Check if we should be polling from the first stream.
        // This means:
        //     - It's our turn to be polled AND the se
        //     - The stream isn't ended
        if this.first_count < &mut this.first_nb_ele.get() {
            if let Some(first) = this.first.as_mut().as_pin_mut() {
                if let Some(item) = ready!(first.poll_next(cx)) {
                    // We have an item! Increment the count for the next poll
                    *this.first_count += 1;
                    return Poll::Ready(Some(item));
                }

                // The stream has finished. Let's dispose of the stream
                this.first.set(None);
            } else {
                // The stream is empty. We can just poll `second` now
                return this
                    .second
                    .as_mut()
                    .as_pin_mut()
                    .map(|second| second.poll_next(cx))
                    .unwrap_or_else(|| Poll::Ready(None));
            }
        }

        // First stream wasn't polled, so we poll the second stream
        if let Some(second) = this.second.as_mut().as_pin_mut() {
            if let Some(item) = ready!(second.poll_next(cx)) {
                // We have an item! Increment the count for the next poll
                *this.second_count += 1;
                return Poll::Ready(Some(item));
            }

            // The stream has finished. Let's dispose of the stream
            this.second.set(None);
        }

        // The second stream is empty. We can just poll `first` now
        this.first
            .as_mut()
            .as_pin_mut()
            .map(|first| first.poll_next(cx))
            .unwrap_or_else(|| Poll::Ready(None))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        match &self.first {
            Some(first) => match &self.second {
                Some(second) => {
                    let first_size = first.size_hint();
                    let second_size = second.size_hint();

                    (
                        first_size.0.saturating_add(second_size.0),
                        match (first_size.1, second_size.1) {
                            (Some(x), Some(y)) => x.checked_add(y),
                            _ => None,
                        },
                    )
                }
                None => first.size_hint(),
            },
            None => match &self.second {
                Some(second) => second.size_hint(),
                None => (0, Some(0)),
            },
        }
    }
}
