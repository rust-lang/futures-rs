//! An unbounded map of futures.
//!
//! This module is only available when the `std` or `alloc` feature of this
//! library is activated, and it is activated by default.

use super::task::Task;
use alloc::sync::Arc;
use core::borrow::Borrow;
use core::fmt::Debug;
use core::hash::{Hash, Hasher};
use core::iter::FromIterator;
use core::pin::Pin;
use core::sync::atomic::Ordering::Relaxed;
use futures_core::future::Future;
use futures_core::stream::Stream;
use futures_core::task::{Context, Poll};
use futures_core::FusedStream;
use std::collections::HashSet;

use super::{FuturesUnordered, IntoIter, IterPinMut, IterPinRef};

/// A map of futures which may complete in any order.
///
/// This structure is optimized to manage a large number of futures.
/// Futures managed by [`MappedFutures`] will only be polled when they
/// generate wake-up notifications. This reduces the required amount of work
/// needed to poll large numbers of futures.
///
/// [`MappedFutures`] can be filled by [`collect`](Iterator::collect)ing an
/// iterator of futures into a [`MappedFutures`], or by
/// [`insert`](MappedFutures::insert)ing futures onto an existing
/// [`MappedFutures`]. When new futures are added,
/// [`poll_next`](Stream::poll_next) must be called in order to begin receiving
/// wake-ups for new futures.
///
/// Note that you can create a ready-made [`MappedFutures`] via the
/// [`collect`](Iterator::collect) method, or you can start with an empty set
/// with the [`MappedFutures::new`] constructor.
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct MappedFutures<K: Hash + Eq, Fut> {
    task_set: HashSet<HashTask<K, HashFut<K, Fut>>>,
    futures: FuturesUnordered<HashFut<K, Fut>>,
}

// Wraps the user-provided Future. Output is associated with a key, partly so that we'll know which
// HashTask to remove from the Set.
#[derive(Debug)]
struct HashFut<K: Hash + Eq, Fut> {
    key: Arc<K>,
    future: Fut,
}

impl<K: Hash + Eq, Fut> HashFut<K, Fut> {
    fn key(&self) -> &K {
        self.key.as_ref()
    }
}

// Wraps the task; but contains a raw pointer, so we need to ensure soundness by ensuring the Task
// is only ever dropped inside release_task(), and is never used after calling release_task()
// Aside from that, HashTask is used to access Task using a key, such as in get_mut, remove,
// cancel, etc.
#[derive(Debug)]
struct HashTask<K: Hash, Fut> {
    inner: *const Task<Fut>,
    key: Arc<K>,
}

impl<K: Hash + Eq, Fut> Borrow<K> for HashTask<K, HashFut<K, Fut>> {
    fn borrow(&self) -> &K {
        &self.key
    }
}

impl<K: Hash + Eq, Fut> PartialEq for HashTask<K, Fut> {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
    }
}

impl<K: Hash + Eq, Fut> Eq for HashTask<K, Fut> {}

impl<K: Hash + Eq, Fut> Hash for HashTask<K, HashFut<K, Fut>> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.key.hash(state);
    }
}

impl<K: Hash + Eq, Fut> HashTask<K, HashFut<K, Fut>> {
    // Extracts some of the unsafety.
    // Get the &mut to the future of the task.
    // The "future not found" case should never occur; the future is removed from task just before
    // task is dropped; consider putting a debug invariant in this function.
    // SAFETY:
    // - we are returning an &mut to the HashFut, requiring only a & ref
    // - this could be used to have multiple mutable references at the same time
    // - use of this function is sound only if there never exists >=2 mut refs to the same future
    // - so, consuming code can either themselves require &mut MappedFutures, and not themselves
    // create multiple mut refs, or can require &MappedFutures, and cast the returned
    // &mut HashFut to a &HashFut
    fn get_future(&self) -> Option<&mut HashFut<K, Fut>> {
        let fut_opt = unsafe { (*(*self.inner).future.get()).as_mut() };
        debug_assert!(fut_opt.is_some());
        fut_opt
    }
}

// SAFETY:
// - the use of Pin::into_inner_unchecked() unchecked is safe because we are only accessing the owned
// future, which is not moved, and its reference is immediaely pinned
// - the other field, of type Arc<K>, is Unpin, and can be moved safely
// - Pin::new_unchecked() is safe because the &mut points to a value that was just also pinned
impl<K: Hash + Eq, Fut: Future> Future for HashFut<K, Fut> {
    type Output = (Arc<K>, Fut::Output);
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let inner = unsafe { Pin::into_inner_unchecked(self) };
        match unsafe { Pin::new_unchecked(&mut inner.future) }.poll(cx) {
            Poll::Ready(res) => Poll::Ready((inner.key.clone(), res)),
            Poll::Pending => Poll::Pending,
        }
    }
}

// SAFETY:
// - all task pointers are owned within MappedFutures, either in FuturesUnordered or in the task
// HashSet
// - so its not possible for a task to be written to while a ready/write is happening, since the
// former op would require &mut access to MappedFutures
unsafe impl<K: Hash + Eq, Fut: Send> Send for MappedFutures<K, Fut> {}
unsafe impl<K: Hash + Eq, Fut: Sync> Sync for MappedFutures<K, Fut> {}

impl<K: Hash + Eq, Fut> Unpin for MappedFutures<K, Fut> {}

impl<K: Hash + Eq, Fut> Default for MappedFutures<K, Fut> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K: Hash + Eq, Fut> MappedFutures<K, Fut> {
    /// Constructs a new, empty [`MappedFutures`].
    ///
    /// The returned [`MappedFutures`] does not contain any futures.
    /// In this state, [`MappedFutures::poll_next`](Stream::poll_next) will
    /// return [`Poll::Ready(None)`](Poll::Ready).
    pub fn new() -> Self {
        Self { task_set: HashSet::new(), futures: FuturesUnordered::new() }
    }

    /// Returns the number of futures contained in the set.
    ///
    /// This represents the total number of in-flight futures.
    pub fn len(&self) -> usize {
        self.futures.len()
    }

    /// Returns `true` if the set contains no futures.
    pub fn is_empty(&self) -> bool {
        // Relaxed ordering can be used here since we don't need to read from
        // the head pointer, only check whether it is null.
        self.futures.is_empty()
    }

    /// Insert a future into the map.
    ///
    /// This method adds the given future to the set. This method will not
    /// call [`poll`](core::future::Future::poll) on the submitted future. The caller must
    /// ensure that [`MappedFutures::poll_next`](Stream::poll_next) is called
    /// in order to receive wake-up notifications for the given future.
    ///
    /// This method will remove and drop a future that is already mapped to the provided key.
    /// Returns true if another future was not removed to make room for the provided future.
    pub fn insert(&mut self, key: K, future: Fut) -> bool {
        let replacing = self.cancel(&key);
        let arc_key = Arc::new(key);
        let hash_fut = HashFut { key: arc_key.clone(), future };
        let task = self.futures.push_inner(hash_fut);
        self.task_set.insert(HashTask { key: arc_key, inner: task });
        !replacing
    }

    /// Insert a future into the set and return the displaced future, if there was one.
    ///
    /// This method adds the given future to the set. This method will not
    /// call [`poll`](core::future::Future::poll) on the submitted future. The caller must
    /// ensure that [`MappedFutures::poll_next`](Stream::poll_next) is called
    /// in order to receive wake-up notifications for the given future.
    /// Returns true if another future was ma
    pub fn replace(&mut self, key: K, future: Fut) -> Option<Fut>
    where
        Fut: Unpin,
    {
        let replacing = self.remove(&key);
        self.insert(key, future);
        replacing
    }

    // Extracts some of the unsafety.
    // Get the &mut to the future of the task.
    // The "future not found" case should never occur; the future is removed from task just before
    // task is dropped; consider putting a debug invariant in this function.
    fn get_task_future(&self, key: &K) -> Option<&mut HashFut<K, Fut>> {
        self.task_set.get(key).and_then(|t| t.get_future())
    }

    /// Remove a future from the set, dropping it.
    ///
    /// Returns true if a future was cancelled.
    pub fn cancel(&mut self, key: &K) -> bool {
        if let Some(task) = self.task_set.take(key) {
            // if task.get_future().is_some() { // unnecessary
            // Should be impossible to get here without task having a future
            // If the future was removed, then task is not in set
            // If future was completed, then it was removed and task dropped
            let unlinked_task = unsafe { self.futures.unlink(task.inner) };
            self.futures.release_task(unlinked_task);
            return true;
        }
        false
    }

    /// Remove a future from the set and return it.
    pub fn remove(&mut self, key: &K) -> Option<Fut>
    where
        Fut: Unpin,
    {
        self.task_set
            .take(key)
            .and_then(|task| unsafe {
                // SAFETY:
                // - If removing the future from task, must do so before releasing task
                // - Derefernce must be safe; if the task had been released then it would have been
                // removed from the set already
                let fut = (*(*task.inner).future.get()).take();
                debug_assert!(fut.is_some());
                let unlinked_task = self.futures.unlink(task.inner);
                self.futures.release_task(unlinked_task);
                fut
            })
            .map(|f| f.future)
    }

    /// Returns `true` if the map contains a future for the specified key.
    pub fn contains(&self, key: &K) -> bool {
        self.task_set.contains(key)
    }

    /// Get a pinned mutable reference to the mapped future.
    pub fn get_pin_mut(&mut self, key: &K) -> Option<Pin<&mut Fut>> {
        self.get_task_future(key).map(|f| unsafe { Pin::new_unchecked(&mut f.future) })
    }

    /// Get a pinned mutable reference to the mapped future.
    pub fn get_mut(&mut self, key: &K) -> Option<&mut Fut>
    where
        Fut: Unpin,
    {
        self.get_task_future(key).map(|f| &mut f.future)
    }

    /// Get a shared reference to the mapped future.
    pub fn get(&self, key: &K) -> Option<&Fut> {
        self.get_task_future(key).map(|f| &f.future)
    }

    /// Get a pinned shared reference to the mapped future.
    pub fn get_pin(&self, key: &K) -> Option<Pin<&Fut>> {
        self.get_task_future(key).map(|f| unsafe { Pin::new_unchecked(&f.future) })
    }

    /// Returns an iterator of keys in the mapping.
    pub fn keys(&self) -> Keys<'_, K, Fut>
    where
        K: Unpin,
    {
        Keys(self.task_set.iter())
        // Keys(Pin::new(self).keys_pin())
    }

    /// Returns an iterator that allows inspecting each future in the set.
    pub fn iter(&self) -> MapIter<'_, K, Fut>
    where
        Fut: Unpin,
    {
        MapIter(Pin::new(self).iter_pin_ref())
    }

    /// Returns an iterator that allows inspecting each future in the set.
    pub fn iter_pin_ref(self: Pin<&Self>) -> MapIterPinRef<'_, K, Fut> {
        MapIterPinRef(unsafe { self.map_unchecked(|f| &f.futures) }.iter_pin_ref())
    }

    /// Returns an iterator that allows modifying each future in the set.
    pub fn iter_mut(&mut self) -> MapIterMut<'_, K, Fut>
    where
        Fut: Unpin,
    {
        MapIterMut(Pin::new(self).iter_pin_mut())
    }

    /// Returns an iterator that allows modifying each future in the set.
    pub fn iter_pin_mut(self: Pin<&mut Self>) -> MapIterPinMut<'_, K, Fut> {
        MapIterPinMut(unsafe { self.map_unchecked_mut(|thing| &mut thing.futures) }.iter_pin_mut())
    }
}

impl<K: Hash + Eq, Fut: Future> Stream for MappedFutures<K, Fut> {
    type Item = (K, Fut::Output);

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match Pin::new(&mut self.futures).poll_next(cx) {
            Poll::Ready(Some(output)) => {
                let key = output.0;
                self.task_set.remove(key.as_ref());
                // Arc::into_inner() only available in >=1.70.0
                // Poll::Ready(Some((Arc::into_inner(key).unwrap(), output.1)))
                //
                // Arc::try_unwrap() is acceptable because keys are only kept 1) in the HashSet,
                // and 2) in the HashFut<Fut>. The complete future has already been dropped here,
                // so the remaining Arc<K> will always have a strong ref count of 1
                Poll::Ready(Some((Arc::try_unwrap(key).ok().unwrap(), output.1)))
            }
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len();
        (len, Some(len))
    }
}

impl<'a, K: Hash + Eq, Fut: Unpin> IntoIterator for &'a MappedFutures<K, Fut> {
    type Item = (&'a K, &'a Fut);
    type IntoIter = MapIter<'a, K, Fut>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, K: Hash + Eq, Fut: Unpin> IntoIterator for &'a mut MappedFutures<K, Fut> {
    type Item = (&'a K, &'a mut Fut);
    type IntoIter = MapIterMut<'a, K, Fut>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

impl<K: Hash + Eq, Fut: Unpin> IntoIterator for MappedFutures<K, Fut> {
    type Item = (K, Fut);
    type IntoIter = MapIntoIter<K, Fut>;

    fn into_iter(self) -> Self::IntoIter {
        MapIntoIter { task_set: self.task_set, inner: self.futures.into_iter() }
    }
}

impl<K: Hash + Eq, Fut> FromIterator<(K, Fut)> for MappedFutures<K, Fut> {
    fn from_iter<I>(iter: I) -> Self
    where
        I: IntoIterator<Item = (K, Fut)>,
    {
        let acc = Self::new();
        iter.into_iter().fold(acc, |mut acc, (key, fut)| {
            acc.insert(key, fut);
            acc
        })
    }
}

impl<K: Hash + Eq, Fut: Future> FusedStream for MappedFutures<K, Fut> {
    fn is_terminated(&self) -> bool {
        self.futures.is_terminated.load(Relaxed)
    }
}

/// Immutable iterator over all keys in the mapping.
#[derive(Debug)]
pub struct Keys<'a, K: Hash + Eq, Fut>(
    std::collections::hash_set::Iter<'a, HashTask<K, HashFut<K, Fut>>>,
);

impl<'a, K: Hash + Eq, Fut> Iterator for Keys<'a, K, Fut> {
    type Item = &'a K;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|hash_task| hash_task.key.as_ref())
    }
}

/// Immutable iterator over all keys in the mapping.
#[derive(Debug)]
pub struct MapIterPinRef<'a, K: Hash + Eq, Fut>(IterPinRef<'a, HashFut<K, Fut>>);

/// Immutable iterator over all keys in the mapping.
#[derive(Debug)]
pub struct MapIterPinMut<'a, K: Hash + Eq, Fut>(IterPinMut<'a, HashFut<K, Fut>>);

/// Mutable iterator over all keys and futures in the map.
#[derive(Debug)]
pub struct MapIterMut<'a, K: Hash + Eq, Fut>(MapIterPinMut<'a, K, Fut>);

/// Immutable iterator over all the keys and futures in the map.
#[derive(Debug)]
pub struct MapIter<'a, K: Hash + Eq, Fut>(MapIterPinRef<'a, K, Fut>);

/// Owned iterator over all keys and futures in the map.
#[derive(Debug)]
pub struct MapIntoIter<K: Hash + Eq, Fut: Unpin> {
    task_set: HashSet<HashTask<K, HashFut<K, Fut>>>,
    inner: IntoIter<HashFut<K, Fut>>,
}

impl<K: Hash + Eq, Fut: Unpin> Iterator for MapIntoIter<K, Fut> {
    type Item = (K, Fut);

    fn next(&mut self) -> Option<Self::Item> {
        let hash_fut = self.inner.next()?;
        self.task_set.remove(hash_fut.key());
        Some((Arc::try_unwrap(hash_fut.key).ok().unwrap(), hash_fut.future))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.inner.len, Some(self.inner.len))
    }
}

impl<'a, K: Hash + Eq, Fut: Unpin> Iterator for MapIterMut<'a, K, Fut> {
    type Item = (&'a K, &'a mut Fut);

    fn next(&mut self) -> Option<Self::Item> {
        let next = self.0.next()?;
        Some((&next.0, Pin::into_inner(next.1)))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

impl<'a, K: Hash + Eq, Fut> Iterator for MapIterPinMut<'a, K, Fut> {
    type Item = (&'a K, Pin<&'a mut Fut>);

    fn next(&mut self) -> Option<Self::Item> {
        let next = unsafe { Pin::into_inner_unchecked(self.0.next()?) };
        Some((&next.key.as_ref(), unsafe { Pin::new_unchecked(&mut next.future) }))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

impl<'a, K: Hash + Eq, Fut> Iterator for MapIterPinRef<'a, K, Fut> {
    type Item = (&'a K, Pin<&'a Fut>);

    fn next(&mut self) -> Option<Self::Item> {
        let next = self.0.next()?;
        let fut = unsafe { next.map_unchecked(|f| &f.future) };
        Some((next.get_ref().key(), fut))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

impl<'a, K: Hash + Eq, Fut: Unpin> Iterator for MapIter<'a, K, Fut> {
    type Item = (&'a K, &'a Fut);

    fn next(&mut self) -> Option<Self::Item> {
        let next = self.0.next()?;
        let key = next.0;
        Some((key, Pin::into_inner(next.1)))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}
impl<K: Hash + Eq, Fut: Unpin> ExactSizeIterator for MapIter<'_, K, Fut> {}
impl<K: Hash + Eq, Fut> ExactSizeIterator for MapIterPinRef<'_, K, Fut> {}
impl<K: Hash + Eq, Fut: Unpin> ExactSizeIterator for MapIterMut<'_, K, Fut> {}
impl<K: Hash + Eq, Fut> ExactSizeIterator for MapIterPinMut<'_, K, Fut> {}
impl<K: Hash + Eq, Fut> ExactSizeIterator for Keys<'_, K, Fut> {}

unsafe impl<K: Hash + Eq, Fut: Send> Send for MapIterPinMut<'_, K, Fut> {}
unsafe impl<K: Hash + Eq, Fut: Sync> Sync for MapIterPinMut<'_, K, Fut> {}

unsafe impl<K: Hash + Eq, Fut: Send> Send for MapIterPinRef<'_, K, Fut> {}
unsafe impl<K: Hash + Eq, Fut: Sync> Sync for MapIterPinRef<'_, K, Fut> {}

/// Tests for MappedFutures
#[cfg(test)]
pub mod tests {
    use crate::stream::*;
    use futures::future::LocalBoxFuture;
    use futures_unordered::mapped_futures::MappedFutures;
    use std::boxed::Box;
    use tokio::time::{sleep, Duration, Instant, Sleep};
    // Two tokio versions available, use the right one
    use tokio_new as tokio;

    fn insert_millis(futs: &mut MappedFutures<u32, Sleep>, key: u32, millis: u64) {
        futs.insert(key, sleep(Duration::from_millis(millis)));
    }

    fn insert_millis_pinned(
        futs: &mut MappedFutures<u32, LocalBoxFuture<'static, ()>>,
        key: u32,
        millis: u64,
    ) {
        futs.insert(key, Box::pin(sleep(Duration::from_millis(millis))));
    }

    #[tokio::test]
    async fn mf_map_futures() {
        let mut futures: MappedFutures<u32, Sleep> = MappedFutures::new();
        insert_millis(&mut futures, 1, 50);
        insert_millis(&mut futures, 2, 75);
        insert_millis(&mut futures, 3, 150);
        insert_millis(&mut futures, 4, 200);

        assert_eq!(futures.next().await.unwrap().0, 1);
        assert!(futures.cancel(&3));
        assert_eq!(futures.next().await.unwrap().0, 2);
        assert_eq!(futures.next().await.unwrap().0, 4);
        assert_eq!(futures.next().await, None);
    }

    #[tokio::test]
    async fn mf_remove_pinned() {
        let mut futures: MappedFutures<u32, LocalBoxFuture<'static, ()>> = MappedFutures::new();
        insert_millis_pinned(&mut futures, 1, 50);
        insert_millis_pinned(&mut futures, 3, 150);
        insert_millis_pinned(&mut futures, 4, 200);

        assert_eq!(futures.next().await.unwrap().0, 1);
        futures.remove(&3).unwrap().await;
        insert_millis_pinned(&mut futures, 2, 60);
        assert_eq!(futures.next().await.unwrap().0, 4);
        assert_eq!(futures.next().await.unwrap().0, 2);
        assert_eq!(futures.next().await, None);
    }

    #[tokio::test]
    async fn mf_mutate() {
        let mut futures: MappedFutures<u32, Sleep> = MappedFutures::new();
        insert_millis(&mut futures, 1, 500);
        insert_millis(&mut futures, 2, 1000);
        insert_millis(&mut futures, 3, 1500);
        insert_millis(&mut futures, 4, 2000);

        assert_eq!(futures.next().await.unwrap().0, 1);
        futures
            .get_pin_mut(&3)
            .unwrap()
            .as_mut()
            .reset(Instant::now() + Duration::from_millis(300));
        assert_eq!(futures.next().await.unwrap().0, 3);
        assert_eq!(futures.next().await.unwrap().0, 2);
        assert_eq!(futures.next().await.unwrap().0, 4);
        assert_eq!(futures.next().await, None);
    }
}
