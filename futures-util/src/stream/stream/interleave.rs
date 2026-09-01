use core::{
    pin::Pin,
    task::{ready, Context, Poll},
};

use futures_core::{FusedStream, Stream};
use pin_project_lite::pin_project;

use crate::stream::Fuse;

pin_project! {
    /// Stream for the [`interleave`](super::StreamExt::interleave) method.
    #[derive(Debug)]
    #[must_use = "streams do nothing unless polled"]
    pub struct Interleave<I, J> {
        #[pin] i: Fuse<I>,
        #[pin] j: Fuse<J>,
        next_coming_from_j: bool,
    }
}

impl<I, J> Interleave<I, J> {
    pub(super) fn new(i: I, j: J) -> Self {
        Self { i: Fuse::new(i), j: Fuse::new(j), next_coming_from_j: false }
    }
    /// Acquires a reference to the underlying streams that this combinator is
    /// pulling from.
    pub fn get_ref(&self) -> (&I, &J) {
        (self.i.get_ref(), self.j.get_ref())
    }

    /// Acquires a mutable reference to the underlying streams that this
    /// combinator is pulling from.
    ///
    /// Note that care must be taken to avoid tampering with the state of the
    /// stream which may otherwise confuse this combinator.
    pub fn get_mut(&mut self) -> (&mut I, &mut J) {
        (self.i.get_mut(), self.j.get_mut())
    }

    /// Acquires a pinned mutable reference to the underlying streams that this
    /// combinator is pulling from.
    ///
    /// Note that care must be taken to avoid tampering with the state of the
    /// stream which may otherwise confuse this combinator.
    pub fn get_pin_mut(self: Pin<&mut Self>) -> (Pin<&mut I>, Pin<&mut J>) {
        let this = self.project();
        (this.i.get_pin_mut(), this.j.get_pin_mut())
    }

    /// Consumes this combinator, returning the underlying streams.
    ///
    /// Note that this may discard intermediate state of this combinator, so
    /// care should be taken to avoid losing resources when this is called.
    pub fn into_inner(self) -> (I, J) {
        (self.i.into_inner(), self.j.into_inner())
    }
}

impl<I: Stream, J: Stream<Item = I::Item>> Stream for Interleave<I, J> {
    type Item = I::Item;
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.project();
        *this.next_coming_from_j = !*this.next_coming_from_j;
        match this.next_coming_from_j {
            true => match ready!(this.i.poll_next(cx)) {
                Some(it) => Poll::Ready(Some(it)),
                None => this.j.poll_next(cx),
            },
            false => match ready!(this.j.poll_next(cx)) {
                Some(it) => Poll::Ready(Some(it)),
                None => this.i.poll_next(cx),
            },
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let (ilo, ihi) = self.i.size_hint();
        let (jlo, jhi) = self.j.size_hint();
        let lo = ilo.saturating_add(jlo);
        let hi = ihi.and_then(|it| it.checked_add(jhi?));
        (lo, hi)
    }
}

impl<I: FusedStream, J: FusedStream<Item = I::Item>> FusedStream for Interleave<I, J> {
    fn is_terminated(&self) -> bool {
        self.i.is_terminated() && self.j.is_terminated()
    }
}
