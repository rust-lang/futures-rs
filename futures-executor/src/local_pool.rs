use crate::park::{Park, ParkDuration, ParkPoll};
use crate::park_thread::ParkThread;
use crate::{enter, Enter};
use futures_core::future::Future;
use futures_core::stream::Stream;
use futures_core::task::{Context, Poll};
use futures_task::{FutureObj, LocalFutureObj, LocalSpawn, Spawn, SpawnError};
use futures_util::stream::FuturesUnordered;
use futures_util::stream::StreamExt;
use std::cell::RefCell;
use std::ops::{Deref, DerefMut};
use std::pin::pin;
use std::rc::{Rc, Weak};
use std::vec::Vec;

// state outside park
#[derive(Debug)]
struct State {
    pool: FuturesUnordered<LocalFutureObj<'static, ()>>,
    incoming: Rc<Incoming>,
}

impl State {
    /// Poll `self.pool`, re-filling it with any newly-spawned tasks.
    /// Repeat until either the pool is empty, or it returns `Pending`.
    ///
    /// Returns `Ready` if the pool was empty, and `Pending` otherwise.
    ///
    /// NOTE: the pool may call `wake`, so `Pending` doesn't necessarily
    /// mean that the pool can't make progress.
    fn poll_pool(&mut self, cx: &mut Context<'_>) -> Poll<()> {
        loop {
            self.drain_incoming();

            let pool_ret = self.pool.poll_next_unpin(cx);

            // We queued up some new tasks; add them and poll again.
            if !self.incoming.borrow().is_empty() {
                continue;
            }

            match pool_ret {
                Poll::Ready(Some(())) => continue,
                Poll::Ready(None) => return Poll::Ready(()),
                Poll::Pending => return Poll::Pending,
            }
        }
    }

    /// Empty the incoming queue of newly-spawned tasks.
    fn drain_incoming(&mut self) {
        let mut incoming = self.incoming.borrow_mut();
        for task in incoming.drain(..) {
            self.pool.push(task)
        }
    }

    /// Runs all tasks and returns after completing one future or until no more progress
    /// can be made. Returns `Ready` if one future was completed, `Pending` otherwise.
    fn poll_pool_one(&mut self, cx: &mut Context<'_>) -> Poll<()> {
        loop {
            self.drain_incoming();

            match self.pool.poll_next_unpin(cx) {
                // Success!
                Poll::Ready(Some(())) => return Poll::Ready(()),
                // The pool was empty.
                Poll::Ready(None) => return Poll::Pending,
                Poll::Pending => (),
            }

            if !self.incoming.borrow().is_empty() {
                // New tasks were spawned; try again.
                continue;
            } else {
                // No more progress for now (but caller should check cx)
                return Poll::Pending;
            }
        }
    }
}

/// A generic single-threaded task pool for polling futures to completion.
///
/// This executor allows you to multiplex any number of tasks onto a single
/// thread. It's appropriate to poll strictly I/O-bound futures that do very
/// little work in between I/O actions.
///
/// To get a handle to the pool that implements
/// [`Spawn`](futures_task::Spawn), use the
/// [`spawner()`](LocalPoolGen::spawner) method. Because the executor is
/// single-threaded, it supports a special form of task spawning for non-`Send`
/// futures, via [`spawn_local_obj`](futures_task::LocalSpawn::spawn_local_obj).
#[derive(Debug)]
pub struct LocalPoolGen<P> {
    park: P,
    state: State,
}

/// A single-threaded task pool for polling futures to completion.
///
/// See [`LocalPoolGen`].
pub type LocalPool = LocalPoolGen<ParkThread>;

/// A handle to a [`LocalPoolGen`] that implements [`Spawn`](futures_task::Spawn).
#[derive(Clone, Debug)]
pub struct LocalSpawner {
    incoming: Weak<Incoming>,
}

type Incoming = RefCell<Vec<LocalFutureObj<'static, ()>>>;

fn local_pool_enter() -> Enter {
    enter().expect("cannot execute `LocalPool` executor from within another executor")
}

// Set up and run a basic task / park loop, polling `f` on each
// turn until it completes and parking between.
fn run_executor<P: Park, T, F: FnMut(&mut Context<'_>) -> Poll<T>>(park: &mut P, mut f: F) -> T {
    let mut enter = local_pool_enter();

    loop {
        let waker = park.waker();
        let mut cx = Context::from_waker(&waker);

        if let Poll::Ready(t) = f(&mut cx) {
            return t;
        }
        park.park(&mut enter, ParkDuration::Block);
    }
}

impl<P> LocalPoolGen<P>
where
    P: Park,
{
    /// Create a new, empty pool of tasks.
    pub fn new() -> Self
    where
        P: Default,
    {
        Self::new_with_park(P::default())
    }

    /// Create a new, empty pool of tasks.
    pub fn new_with_park(park: P) -> Self {
        Self { park, state: State { pool: FuturesUnordered::new(), incoming: Default::default() } }
    }

    /// Returns a reference to the underlying Park instance.
    pub fn get_park(&self) -> &P {
        &self.park
    }

    /// Returns a mutable reference to the underlying Park instance.
    pub fn get_park_mut(&mut self) -> &mut P {
        &mut self.park
    }

    /// Get a clonable handle to the pool as a [`Spawn`].
    pub fn spawner(&self) -> LocalSpawner {
        LocalSpawner { incoming: Rc::downgrade(&self.state.incoming) }
    }

    /// Run all tasks in the pool to completion.
    ///
    /// ```
    /// use futures::executor::LocalPool;
    ///
    /// let mut pool = LocalPool::new();
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
        let park = &mut self.park;
        let state = &mut self.state;
        run_executor(park, |cx| state.poll_pool(cx))
    }

    /// Runs all the tasks in the pool until the given future completes.
    ///
    /// ```
    /// use futures::executor::LocalPool;
    ///
    /// let mut pool = LocalPool::new();
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
        let mut future = pin!(future);
        let park = &mut self.park;
        let state = &mut self.state;

        run_executor(park, |cx| {
            // if our main task is done, so are we
            let result = future.as_mut().poll(cx);
            if let Poll::Ready(output) = result {
                return Poll::Ready(output);
            }

            let _ = state.poll_pool(cx);
            Poll::Pending
        })
    }
}

impl<P> LocalPoolGen<P>
where
    P: ParkPoll,
{
    /// Runs all tasks and returns after completing one future or until no more progress
    /// can be made. Returns `true` if one future was completed, `false` otherwise.
    ///
    /// ```
    /// use futures::executor::LocalPool;
    /// use futures::task::LocalSpawnExt;
    /// use futures::future::{ready, pending};
    ///
    /// let mut pool = LocalPool::new();
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
        let mut enter = local_pool_enter();

        loop {
            let waker = self.park.waker();
            let mut cx = Context::from_waker(&waker);

            // progress as much as needed to finish single task
            if let Poll::Ready(()) = self.state.poll_pool_one(&mut cx) {
                // finished single task
                return true;
            }
            if self.park.poll(&mut enter).is_pending() {
                // can't make more progress; park() would block now.
                return false;
            }
        }
    }

    /// Runs all tasks in the pool and returns if no more progress can be made
    /// on any task.
    ///
    /// ```
    /// use futures::executor::LocalPool;
    /// use futures::task::LocalSpawnExt;
    /// use futures::future::{ready, pending};
    ///
    /// let mut pool = LocalPool::new();
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
        let mut enter = local_pool_enter();

        loop {
            if self.poll(&mut enter).is_pending() {
                // can't make more progress; park() would block now.
                return;
            }
        }
    }
}

impl<P: Park + Default> Default for LocalPoolGen<P> {
    fn default() -> Self {
        Self::new_with_park(P::default())
    }
}

impl<P: Park> Park for LocalPoolGen<P> {
    fn waker(&self) -> futures_task::WakerRef<'_> {
        self.park.waker()
    }

    fn park(&mut self, enter: &mut crate::Enter, duration: ParkDuration) {
        let waker = self.park.waker();
        let mut cx = Context::from_waker(&waker);

        // make as much progress as possible; if this "completes" (`Ready`)
        // the task pool is empty, if it just yielded it will have triggered
        // the waker.
        let _ = self.state.poll_pool(&mut cx);

        self.park.park(enter, duration);
    }
}

impl<P: ParkPoll> ParkPoll for LocalPoolGen<P> {
    fn poll(&mut self, enter: &mut Enter) -> Poll<()> {
        let waker = self.park.waker();
        let mut cx = Context::from_waker(&waker);

        // make as much progress as possible; if this "completes" (`Ready`)
        // the task pool is empty, if it just yielded it will have triggered
        // the waker.
        let _ = self.state.poll_pool(&mut cx);

        self.park.poll(enter)
    }
}

/// Run a future to completion on the current thread.
///
/// This function will block the caller until the given future has completed.
///
/// Use a [`LocalPoolGen`] if you need finer-grained control over spawned tasks.
pub fn block_on<F: Future>(f: F) -> F::Output {
    let mut f = pin!(f);
    run_executor(&mut ParkThread::new(), |cx| f.as_mut().poll(cx))
}

/// Turn a stream into a blocking iterator.
///
/// When `next` is called on the resulting `BlockingStream`, the caller
/// will be blocked until the next element of the `Stream` becomes available.
pub fn block_on_stream<S: Stream + Unpin>(stream: S) -> BlockingStream<S> {
    BlockingStream { stream }
}

/// An iterator which blocks on values from a stream until they become available.
#[derive(Debug)]
pub struct BlockingStream<S: Stream + Unpin> {
    stream: S,
}

impl<S: Stream + Unpin> Deref for BlockingStream<S> {
    type Target = S;
    fn deref(&self) -> &Self::Target {
        &self.stream
    }
}

impl<S: Stream + Unpin> DerefMut for BlockingStream<S> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.stream
    }
}

impl<S: Stream + Unpin> BlockingStream<S> {
    /// Convert this `BlockingStream` into the inner `Stream` type.
    pub fn into_inner(self) -> S {
        self.stream
    }
}

impl<S: Stream + Unpin> Iterator for BlockingStream<S> {
    type Item = S::Item;

    fn next(&mut self) -> Option<Self::Item> {
        LocalPool::new().run_until(self.stream.next())
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
