use super::task::Task;
use super::{FuturesUnorderedInternal, ReleasesTask};
use core::marker::PhantomData;
use core::pin::Pin;
use core::ptr;
use core::sync::atomic::Ordering::Relaxed;

/// Mutable iterator over all futures in the unordered set.
#[derive(Debug)]
pub(crate) struct IterPinMut<'a, K, Fut, S: ReleasesTask<K>> {
    pub(super) task: *const Task<K, Fut>,
    pub(crate) len: usize,
    pub(super) _marker: PhantomData<&'a mut FuturesUnorderedInternal<K, Fut, S>>,
}

/// Mutable iterator over all futures in the unordered set.
#[derive(Debug)]
pub(crate) struct IterMut<'a, K, Fut: Unpin, S: ReleasesTask<K>>(
    pub(super) IterPinMut<'a, K, Fut, S>,
);

/// Immutable iterator over all futures in the unordered set.
#[derive(Debug)]
pub(crate) struct IterPinRef<'a, K, Fut, S: ReleasesTask<K>> {
    pub(super) task: *const Task<K, Fut>,
    pub(crate) len: usize,
    pub(super) pending_next_all: *mut Task<K, Fut>,
    pub(super) _marker: PhantomData<&'a FuturesUnorderedInternal<K, Fut, S>>,
}

/// Immutable iterator over all the futures in the unordered set.
#[derive(Debug)]
pub(crate) struct Iter<'a, K, Fut: Unpin, S: ReleasesTask<K>>(pub(super) IterPinRef<'a, K, Fut, S>);

/// Owned iterator over all futures in the unordered set.
#[derive(Debug)]
pub(crate) struct IntoIter<K, Fut: Unpin, S: ReleasesTask<K>> {
    pub(crate) len: usize,
    pub(super) inner: FuturesUnorderedInternal<K, Fut, S>,
}

impl<K, Fut: Unpin, S: ReleasesTask<K>> Iterator for IntoIter<K, Fut, S> {
    type Item = (K, Fut);

    fn next(&mut self) -> Option<Self::Item> {
        // `head_all` can be accessed directly and we don't need to spin on
        // `Task::next_all` since we have exclusive access to the set.
        let task = self.inner.head_all.get_mut();

        if (*task).is_null() {
            return None;
        }

        unsafe {
            // Moving out of the future is safe because it is `Unpin`
            let future = (*(**task).future.get()).take().unwrap();
            let key = (**task).take_key();

            // Mutable access to a previously shared `FuturesUnorderedInternal` implies
            // that the other threads already released the object before the
            // current thread acquired it, so relaxed ordering can be used and
            // valid `next_all` checks can be skipped.
            let next = (**task).next_all.load(Relaxed);
            *task = next;
            if !task.is_null() {
                *(**task).prev_all.get() = ptr::null_mut();
            }
            self.len -= 1;
            Some((key, future))
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len, Some(self.len))
    }
}

impl<K, Fut: Unpin, S: ReleasesTask<K>> ExactSizeIterator for IntoIter<K, Fut, S> {}

impl<'a, K, Fut, S: ReleasesTask<K>> Iterator for IterPinMut<'a, K, Fut, S> {
    type Item = (&'a K, Pin<&'a mut Fut>);

    fn next(&mut self) -> Option<Self::Item> {
        if self.task.is_null() {
            return None;
        }

        unsafe {
            let future = (*(*self.task).future.get()).as_mut().unwrap();
            let key = (*(*self.task).key.get()).as_ref().unwrap();

            // Mutable access to a previously shared `FuturesUnorderedInternal` implies
            // that the other threads already released the object before the
            // current thread acquired it, so relaxed ordering can be used and
            // valid `next_all` checks can be skipped.
            let next = (*self.task).next_all.load(Relaxed);
            self.task = next;
            self.len -= 1;
            Some((key, Pin::new_unchecked(future)))
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len, Some(self.len))
    }
}

impl<K, Fut, S: ReleasesTask<K>> ExactSizeIterator for IterPinMut<'_, K, Fut, S> {}

impl<'a, K, Fut: Unpin, S: ReleasesTask<K>> Iterator for IterMut<'a, K, Fut, S> {
    type Item = (&'a K, &'a mut Fut);

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|opt| (opt.0, Pin::get_mut(opt.1)))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

impl<K, Fut: Unpin, S: ReleasesTask<K>> ExactSizeIterator for IterMut<'_, K, Fut, S> {}

impl<'a, K, Fut, S: ReleasesTask<K>> Iterator for IterPinRef<'a, K, Fut, S> {
    type Item = (&'a K, Pin<&'a Fut>);

    fn next(&mut self) -> Option<Self::Item> {
        if self.task.is_null() {
            return None;
        }

        unsafe {
            let future = (*(*self.task).future.get()).as_ref().unwrap();
            let key = (*(*self.task).key.get()).as_ref().unwrap();

            // Relaxed ordering can be used since acquire ordering when
            // `head_all` was initially read for this iterator implies acquire
            // ordering for all previously inserted nodes (and we don't need to
            // read `len_all` again for any other nodes).
            let next = (*self.task).spin_next_all(self.pending_next_all, Relaxed);
            self.task = next;
            self.len -= 1;
            Some((key, Pin::new_unchecked(future)))
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len, Some(self.len))
    }
}

impl<K, Fut, S: ReleasesTask<K>> ExactSizeIterator for IterPinRef<'_, K, Fut, S> {}

impl<'a, K, Fut: Unpin, S: ReleasesTask<K>> Iterator for Iter<'a, K, Fut, S> {
    type Item = (&'a K, &'a Fut);

    fn next(&mut self) -> Option<Self::Item> {
        // self.0.next().map(Pin::get_ref)
        self.0.next().map(|opt| (opt.0, Pin::get_ref(opt.1)))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

impl<K, Fut: Unpin, S: ReleasesTask<K>> ExactSizeIterator for Iter<'_, K, Fut, S> {}

// SAFETY: we do nothing thread-local and there is no interior mutability,
// so the usual structural `Send`/`Sync` apply.
unsafe impl<K: Send, Fut: Send, S: ReleasesTask<K>> Send for IterPinRef<'_, K, Fut, S> {}
unsafe impl<K: Sync, Fut: Sync, S: ReleasesTask<K>> Sync for IterPinRef<'_, K, Fut, S> {}

unsafe impl<K: Send, Fut: Send, S: ReleasesTask<K>> Send for IterPinMut<'_, K, Fut, S> {}
unsafe impl<K: Sync, Fut: Sync, S: ReleasesTask<K>> Sync for IterPinMut<'_, K, Fut, S> {}

unsafe impl<K: Send, Fut: Send + Unpin, S: ReleasesTask<K>> Send for IntoIter<K, Fut, S> {}
unsafe impl<K: Sync, Fut: Sync + Unpin, S: ReleasesTask<K>> Sync for IntoIter<K, Fut, S> {}
