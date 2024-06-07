use crate::stream::futures_unordered_internal;
use core::pin::Pin;

use super::DummyStruct;

/// Mutable iterator over all futures in the unordered set.
#[derive(Debug)]
pub struct IterPinMut<'a, Fut> {
    pub(super) inner: futures_unordered_internal::IterPinMut<'a, (), Fut, DummyStruct>,
}

/// Mutable iterator over all futures in the unordered set.
#[derive(Debug)]
pub struct IterMut<'a, Fut: Unpin>(pub(super) IterPinMut<'a, Fut>);

/// Immutable iterator over all futures in the unordered set.
#[derive(Debug)]
pub struct IterPinRef<'a, Fut> {
    pub(super) inner: futures_unordered_internal::IterPinRef<'a, (), Fut, DummyStruct>,
}

/// Immutable iterator over all the futures in the unordered set.
#[derive(Debug)]
pub struct Iter<'a, Fut: Unpin>(pub(super) IterPinRef<'a, Fut>);

/// Owned iterator over all futures in the unordered set.
#[derive(Debug)]
pub struct IntoIter<Fut: Unpin> {
    pub(super) inner: futures_unordered_internal::IntoIter<(), Fut, DummyStruct>,
}

impl<Fut: Unpin> Iterator for IntoIter<Fut> {
    type Item = Fut;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|opt| opt.1)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.inner.len, Some(self.inner.len))
    }
}

impl<Fut: Unpin> ExactSizeIterator for IntoIter<Fut> {}

impl<'a, Fut> Iterator for IterPinMut<'a, Fut> {
    type Item = Pin<&'a mut Fut>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|opt| opt.1)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.inner.len, Some(self.inner.len))
    }
}

impl<Fut> ExactSizeIterator for IterPinMut<'_, Fut> {}

impl<'a, Fut: Unpin> Iterator for IterMut<'a, Fut> {
    type Item = &'a mut Fut;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(Pin::get_mut)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

impl<Fut: Unpin> ExactSizeIterator for IterMut<'_, Fut> {}

impl<'a, Fut> Iterator for IterPinRef<'a, Fut> {
    type Item = Pin<&'a Fut>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|opt| opt.1)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.inner.len, Some(self.inner.len))
    }
}

impl<Fut> ExactSizeIterator for IterPinRef<'_, Fut> {}

impl<'a, Fut: Unpin> Iterator for Iter<'a, Fut> {
    type Item = &'a Fut;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(Pin::get_ref)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

impl<Fut: Unpin> ExactSizeIterator for Iter<'_, Fut> {}
