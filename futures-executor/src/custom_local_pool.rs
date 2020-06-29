//! `no_std` local task execution.
//!
//! This module is only available when the `alloc` feature of this library is
//! activated, and it is activated by default.

use alloc::rc::{Rc, Weak};
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::cell::RefCell;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::{AtomicBool, Ordering};
use futures_core::future::Future;
use futures_core::stream::Stream;
use futures_core::task::{Context, Poll};
use futures_task::{waker_ref, ArcWake};
use futures_task::{FutureObj, LocalFutureObj, LocalSpawn, Spawn, SpawnError};
use futures_util::pin_mut;
use futures_util::stream::FuturesUnordered;
use futures_util::stream::StreamExt;

/// A custom single-threaded task pool for polling futures to completion in
/// `no_std` environments.
///
/// This executor allows you to multiplex any number of tasks onto a single
/// thread. It's appropriate to poll strictly I/O-bound futures that do very
/// little work in between I/O actions.
///
/// To get a handle to the pool that implements
/// [`Spawn`](futures_task::Spawn), use the
/// [`spawner()`](LocalPool::spawner) method. Because the executor is
/// single-threaded, it supports a special form of task spawning for non-`Send`
/// futures, via [`spawn_local_obj`](futures_task::LocalSpawn::spawn_local_obj).
#[derive(Debug)]
pub struct LocalPool<P, U> {
    pool: FuturesUnordered<LocalFutureObj<'static, ()>>,
    incoming: Rc<Incoming>,
    park: P,
    notify: Arc<Notify<U>>,
}

/// A handle to a [`LocalPool`](LocalPool) that implements
/// [`Spawn`](futures_task::Spawn).
#[derive(Clone, Debug)]
pub struct LocalSpawner {
    incoming: Weak<Incoming>,
}

type Incoming = RefCell<Vec<LocalFutureObj<'static, ()>>>;

#[derive(Debug)]
struct Notify<U> {
    /// The function to unpark the current thread.
    unpark: U,
    /// A flag to ensure a wakeup (i.e. `unpark()`) is not "forgotten"
    /// before the next `park()`, which may otherwise happen if the code
    /// being executed as part of the future(s) being polled makes use of
    /// park / unpark calls of its own, i.e. we cannot assume that no other
    /// code uses park / unpark on the executing `thread`.
    unparked: AtomicBool,
}

impl<U: Fn() + Send + Sync> ArcWake for Notify<U> {
    fn wake_by_ref(arc_self: &Arc<Self>) {
        // Make sure the wakeup is remembered until the next `park()`.
        let unparked = arc_self.unparked.swap(true, Ordering::Relaxed);
        if !unparked {
            // If the thread has not been unparked yet, it must be done
            // now. If it was actually parked, it will run again,
            // otherwise the token made available by `unpark`
            // may be consumed before reaching `park()`, but `unparked`
            // ensures it is not forgotten.
            (arc_self.unpark)();
        }
    }
}

// Set up and run a basic single-threaded spawner loop, invoking `f` on each
// turn.
fn run_executor<T, F: FnMut(&mut Context<'_>) -> Poll<T>, P: Fn(), U: Fn() + Send + Sync>(
    mut f: F,
    park: P,
    notify: &Arc<Notify<U>>,
) -> T {
    let waker = waker_ref(notify);
    let mut cx = Context::from_waker(&waker);
    loop {
        if let Poll::Ready(t) = f(&mut cx) {
            return t;
        }
        // Consume the wakeup that occurred while executing `f`, if any.
        let unparked = notify.unparked.swap(false, Ordering::Acquire);
        if !unparked {
            // No wakeup occurred. It may occur now, right before parking,
            // but in that case the token made available by `unpark()`
            // is guaranteed to still be available and `park()` is a no-op.
            park();
            // When the thread is unparked, `unparked` will have been set
            // and needs to be unset before the next call to `f` to avoid
            // a redundant loop iteration.
            notify.unparked.store(false, Ordering::Release);
        }
    }
}

fn poll_executor<T, F: FnMut(&mut Context<'_>) -> T, U: Fn() + Send + Sync>(
    mut f: F,
    notify: &Arc<Notify<U>>,
) -> T {
    let waker = waker_ref(notify);
    let mut cx = Context::from_waker(&waker);
    f(&mut cx)
}

impl<P: Fn() + Clone, U: Fn() + Clone + Send + Sync> LocalPool<P, U> {
    /// Create a new, empty pool of tasks.
    ///
    /// ```ignore
    /// use cortex_m::asm;
    /// use futures::executor::custom_local_pool::LocalPool;
    ///
    /// let pool = LocalPool::new(asm::wfe, asm::sev);
    /// ```
    pub fn new(park: P, unpark: U) -> Self {
        Self {
            pool: FuturesUnordered::new(),
            incoming: Default::default(),
            park,
            notify: Arc::new(Notify {
                unpark,
                unparked: AtomicBool::new(false),
            }),
        }
    }

    /// Get a clonable handle to the pool as a [`Spawn`].
    pub fn spawner(&self) -> LocalSpawner {
        LocalSpawner {
            incoming: Rc::downgrade(&self.incoming),
        }
    }

    /// Run all tasks in the pool to completion.
    ///
    /// ```ignore
    /// use cortex_m::asm;
    /// use futures::executor::custom_local_pool::LocalPool;
    ///
    /// let pool = LocalPool::new(asm::wfe, asm::sev);
    ///
    /// // ... spawn some initial tasks using `spawn.spawn()` or `spawn.spawn_local()`
    ///
    /// // run *all* tasks in the pool to completion, including any newly-spawned ones.
    /// pool.run();
    /// ```
    ///
    /// The function will block the calling thread until *all* tasks in the pool
    /// are complete, including any spawned while running existing tasks.
    pub fn run(&mut self) {
        let park = self.park.clone();
        let notify = self.notify.clone();
        run_executor(|cx| self.poll_pool(cx), park, &notify)
    }

    /// Runs all the tasks in the pool until the given future completes.
    ///
    /// ```ignore
    /// use cortex_m::asm;
    /// use futures::executor::custom_local_pool::LocalPool;
    ///
    /// let pool = LocalPool::new(asm::wfe, asm::sev);
    /// # let my_app  = async {};
    ///
    /// // run tasks in the pool until `my_app` completes
    /// pool.run_until(my_app);
    /// ```
    ///
    /// The function will block the calling thread *only* until the future `f`
    /// completes; there may still be incomplete tasks in the pool, which will
    /// be inert after the call completes, but can continue with further use of
    /// one of the pool's run or poll methods. While the function is running,
    /// however, all tasks in the pool will try to make progress.
    pub fn run_until<F: Future>(&mut self, future: F) -> F::Output {
        let park = self.park.clone();
        let notify = self.notify.clone();

        pin_mut!(future);

        run_executor(
            |cx| {
                {
                    // if our main task is done, so are we
                    let result = future.as_mut().poll(cx);
                    if let Poll::Ready(output) = result {
                        return Poll::Ready(output);
                    }
                }

                let _ = self.poll_pool(cx);
                Poll::Pending
            },
            park,
            &notify,
        )
    }

    /// Runs all tasks and returns after completing one future or until no more progress
    /// can be made. Returns `true` if one future was completed, `false` otherwise.
    ///
    /// ```
    /// use cortex_m::asm;
    /// use futures::executor::custom_local_pool::LocalPool;
    /// use futures::task::LocalSpawnExt;
    /// use futures::future::{ready, pending};
    ///
    /// let pool = LocalPool::new(asm::wfe, asm::sev);
    /// let spawner = pool.spawner();
    ///
    /// spawner.spawn_local(ready(())).unwrap();
    /// spawner.spawn_local(ready(())).unwrap();
    /// spawner.spawn_local(pending()).unwrap();
    ///
    /// // Run the two ready tasks and return true for them.
    /// pool.try_run_one(); // returns true after completing one of the ready futures
    /// pool.try_run_one(); // returns true after completing the other ready future
    ///
    /// // the remaining task can not be completed
    /// assert!(!pool.try_run_one()); // returns false
    /// ```
    ///
    /// This function will not block the calling thread and will return the moment
    /// that there are no tasks left for which progress can be made or after exactly one
    /// task was completed; Remaining incomplete tasks in the pool can continue with
    /// further use of one of the pool's run or poll methods.
    /// Though only one task will be completed, progress may be made on multiple tasks.
    pub fn try_run_one(&mut self) -> bool {
        let notify = self.notify.clone();

        poll_executor(
            |ctx| {
                loop {
                    let ret = self.poll_pool_once(ctx);

                    // return if we have executed a future
                    if let Poll::Ready(Some(_)) = ret {
                        return true;
                    }

                    // if there are no new incoming futures
                    // then there is no feature that can make progress
                    // and we can return without having completed a single future
                    if self.incoming.borrow().is_empty() {
                        return false;
                    }
                }
            },
            &notify,
        )
    }

    /// Runs all tasks in the pool and returns if no more progress can be made
    /// on any task.
    ///
    /// ```ignore
    /// use cortex_m::asm;
    /// use futures::executor::custom_local_pool::LocalPool;
    /// use futures::task::LocalSpawnExt;
    /// use futures::future::{ready, pending};
    ///
    /// let pool = LocalPool::new(asm::wfe, asm::sev);
    /// let spawner = pool.spawner();
    ///
    /// spawner.spawn_local(ready(())).unwrap();
    /// spawner.spawn_local(ready(())).unwrap();
    /// spawner.spawn_local(pending()).unwrap();
    ///
    /// // Runs the two ready task and returns.
    /// // The empty task remains in the pool.
    /// pool.run_until_stalled();
    /// ```
    ///
    /// This function will not block the calling thread and will return the moment
    /// that there are no tasks left for which progress can be made;
    /// remaining incomplete tasks in the pool can continue with further use of one
    /// of the pool's run or poll methods. While the function is running, all tasks
    /// in the pool will try to make progress.
    pub fn run_until_stalled(&mut self) {
        let notify = self.notify.clone();

        poll_executor(
            |ctx| {
                let _ = self.poll_pool(ctx);
            },
            &notify,
        );
    }

    // Make maximal progress on the entire pool of spawned task, returning `Ready`
    // if the pool is empty and `Pending` if no further progress can be made.
    fn poll_pool(&mut self, cx: &mut Context<'_>) -> Poll<()> {
        // state for the FuturesUnordered, which will never be used
        loop {
            let ret = self.poll_pool_once(cx);

            // we queued up some new tasks; add them and poll again
            if !self.incoming.borrow().is_empty() {
                continue;
            }

            // no queued tasks; we may be done
            match ret {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(None) => return Poll::Ready(()),
                _ => {}
            }
        }
    }

    // Try make minimal progress on the pool of spawned tasks
    fn poll_pool_once(&mut self, cx: &mut Context<'_>) -> Poll<Option<()>> {
        // empty the incoming queue of newly-spawned tasks
        {
            let mut incoming = self.incoming.borrow_mut();
            for task in incoming.drain(..) {
                self.pool.push(task)
            }
        }

        // try to execute the next ready future
        self.pool.poll_next_unpin(cx)
    }

    /// Run a future to completion on the current thread.
    ///
    /// This function will block the caller until the given future has completed.
    ///
    /// Use a [`LocalPool`](LocalPool) if you need finer-grained control over
    /// spawned tasks.
    pub fn block_on<F: Future>(self: &mut Self, f: F) -> F::Output {
        let park = self.park.clone();
        let notify = self.notify.clone();

        pin_mut!(f);
        run_executor(|cx| f.as_mut().poll(cx), park, &notify)
    }

    /// Turn a stream into a blocking iterator.
    ///
    /// When `next` is called on the resulting `BlockingStream`, the caller
    /// will be blocked until the next element of the `Stream` becomes available.
    pub fn block_on_stream<S: Stream + Unpin>(
        self: &mut Self,
        stream: S,
    ) -> BlockingStream<'_, S, P, U> {
        BlockingStream { pool: self, stream }
    }
}

/// An iterator which blocks on values from a stream until they become available.
#[derive(Debug)]
pub struct BlockingStream<'a, S: Stream + Unpin, P: Fn(), U: Fn()> {
    pool: &'a mut LocalPool<P, U>,
    stream: S,
}

impl<S: Stream + Unpin, P: Fn(), U: Fn()> Deref for BlockingStream<'_, S, P, U> {
    type Target = S;
    fn deref(&self) -> &Self::Target {
        &self.stream
    }
}

impl<S: Stream + Unpin, P: Fn(), U: Fn()> DerefMut for BlockingStream<'_, S, P, U> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.stream
    }
}

impl<S: Stream + Unpin, P: Fn(), U: Fn()> BlockingStream<'_, S, P, U> {
    /// Convert this `BlockingStream` into the inner `Stream` type.
    pub fn into_inner(self) -> S {
        self.stream
    }
}

impl<S, P, U> Iterator for BlockingStream<'_, S, P, U>
where
    S: Stream + Unpin,
    P: Fn() + Clone,
    U: Fn() + Clone + Send + Sync,
{
    type Item = S::Item;

    fn next(&mut self) -> Option<Self::Item> {
        self.pool.run_until(self.stream.next())
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.stream.size_hint()
    }
}

impl Spawn for LocalSpawner {
    fn spawn_obj(&self, future: FutureObj<'static, ()>) -> Result<(), SpawnError> {
        if let Some(incoming) = self.incoming.upgrade() {
            incoming.borrow_mut().push(future.into());
            Ok(())
        } else {
            Err(SpawnError::shutdown())
        }
    }

    fn status(&self) -> Result<(), SpawnError> {
        if self.incoming.upgrade().is_some() {
            Ok(())
        } else {
            Err(SpawnError::shutdown())
        }
    }
}

impl LocalSpawn for LocalSpawner {
    fn spawn_local_obj(&self, future: LocalFutureObj<'static, ()>) -> Result<(), SpawnError> {
        if let Some(incoming) = self.incoming.upgrade() {
            incoming.borrow_mut().push(future);
            Ok(())
        } else {
            Err(SpawnError::shutdown())
        }
    }

    fn status_local(&self) -> Result<(), SpawnError> {
        if self.incoming.upgrade().is_some() {
            Ok(())
        } else {
            Err(SpawnError::shutdown())
        }
    }
}
