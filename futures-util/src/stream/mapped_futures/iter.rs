use super::TaskSet;
use crate::stream::futures_keyed;
use core::hash::Hash;
use core::pin::Pin;

/// Mutable iterator over all futures in the unordered set.
// #[derive(Debug)]
pub struct IterPinMut<'a, K: Hash + Eq, Fut> {
    pub(super) inner: futures_keyed::IterPinMut<'a, K, Fut, TaskSet<K, Fut>>,
    // pub(super) task: *const Task<K, Fut>,
    // pub(super) len: usize,
    // pub(super) _marker: PhantomData<&'a mut MappedFutures<K, Fut>>,
}

/// Mutable iterator over all futures in the unordered set.
// #[derive(Debug)]
pub struct IterMut<'a, K: Hash + Eq, Fut: Unpin>(pub(super) IterPinMut<'a, K, Fut>);

/// Immutable iterator over all futures in the unordered set.
// #[derive(Debug)]
pub struct IterPinRef<'a, K: Hash + Eq, Fut> {
    // pub(super) task: *const Task<K, Fut>,
    // pub(super) len: usize,
    // pub(super) pending_next_all: *mut Task<K, Fut>,
    // pub(super) _marker: PhantomData<&'a MappedFutures<K, Fut>>,
    pub(super) inner: futures_keyed::IterPinRef<'a, K, Fut, TaskSet<K, Fut>>,
}

/// Immutable iterator over all the futures in the unordered set.
// #[derive(Debug)]
pub struct Iter<'a, K: Hash + Eq, Fut: Unpin>(pub(super) IterPinRef<'a, K, Fut>);

/// Owned iterator over all futures in the unordered set.
// #[derive(Debug)]
pub struct IntoIter<K: Hash + Eq, Fut: Unpin> {
    pub(super) inner: futures_keyed::IntoIter<K, Fut, TaskSet<K, Fut>>,
}

/// Immutable iterator over all keys in the mapping.
pub struct Keys<'a, K: Hash + Eq, Fut> {
    pub(super) inner: futures_keyed::IterPinRef<'a, K, Fut, TaskSet<K, Fut>>, // pub(super) inner: std::iter::Map<
                                                                              //     std::collections::hash_set::Iter<'a, HashTask<K, Fut>>,
                                                                              //     Box<dyn FnMut(&'a HashTask<K, Fut>) -> &'a K>,
                                                                              // >,
}

impl<K: Hash + Eq, Fut: Unpin> Iterator for IntoIter<K, Fut> {
    type Item = (K, Fut);

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl<K: Hash + Eq, Fut: Unpin> ExactSizeIterator for IntoIter<K, Fut> {}

impl<'a, K: Hash + Eq, Fut> Iterator for IterPinMut<'a, K, Fut> {
    type Item = (&'a K, Pin<&'a mut Fut>);

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.inner.len, Some(self.inner.len))
    }
}

impl<K: Hash + Eq, Fut> ExactSizeIterator for IterPinMut<'_, K, Fut> {}

impl<'a, K: Hash + Eq, Fut: Unpin> Iterator for IterMut<'a, K, Fut> {
    type Item = (&'a K, &'a mut Fut);

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|(key, fut_pin)| (key, Pin::get_mut(fut_pin)))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

impl<K: Hash + Eq, Fut: Unpin> ExactSizeIterator for IterMut<'_, K, Fut> {}

impl<'a, K: Hash + Eq, Fut> Iterator for IterPinRef<'a, K, Fut> {
    type Item = (&'a K, Pin<&'a Fut>);

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl<K: Hash + Eq, Fut> ExactSizeIterator for IterPinRef<'_, K, Fut> {}

impl<'a, K: Hash + Eq, Fut: Unpin> Iterator for Iter<'a, K, Fut> {
    type Item = (&'a K, &'a Fut);

    fn next(&mut self) -> Option<Self::Item> {
        // self.0.next()
        self.0.next().map(|(key, fut_pin)| (key, Pin::get_ref(fut_pin)))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

impl<K: Hash + Eq, Fut: Unpin> ExactSizeIterator for Iter<'_, K, Fut> {}

impl<K: Hash + Eq, Fut: Unpin> ExactSizeIterator for Keys<'_, K, Fut> {}

impl<'a, K: Hash + Eq, Fut> Iterator for Keys<'a, K, Fut> {
    type Item = &'a K;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|opt| opt.0)
    }
}

// SAFETY: we do nothing thread-local and there is no interior mutability,
// so the usual structural `Send`/`Sync` apply.
unsafe impl<K: Hash + Eq, Fut: Send> Send for IterPinRef<'_, K, Fut> {}
unsafe impl<K: Hash + Eq, Fut: Sync> Sync for IterPinRef<'_, K, Fut> {}

unsafe impl<K: Hash + Eq, Fut: Send> Send for IterPinMut<'_, K, Fut> {}
unsafe impl<K: Hash + Eq, Fut: Sync> Sync for IterPinMut<'_, K, Fut> {}

unsafe impl<K: Hash + Eq, Fut: Send + Unpin> Send for IntoIter<K, Fut> {}
unsafe impl<K: Hash + Eq, Fut: Sync + Unpin> Sync for IntoIter<K, Fut> {}

unsafe impl<K: Hash + Eq, Fut: Send + Unpin> Send for Keys<'_, K, Fut> {}
unsafe impl<K: Hash + Eq, Fut: Sync + Unpin> Sync for Keys<'_, K, Fut> {}
