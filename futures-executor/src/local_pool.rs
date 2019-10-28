use crate::enter;
use crate::park::{Park, ParkDuration};
use crate::park_thread::ParkThread;
use futures_core::future::{Future, FutureObj, LocalFutureObj};
use futures_core::stream::{Stream};
use futures_core::task::{Context, Poll, Spawn, LocalSpawn, SpawnError};
use futures_util::stream::FuturesUnordered;
use futures_util::stream::StreamExt;
use futures_util::pin_mut;
use std::cell::{RefCell};
use std::ops::{Deref, DerefMut};
use std::rc::{Rc, Weak};

// state outside park
#[derive(Debug)]
struct State {
    pool: FuturesUnordered<LocalFutureObj<'static, ()>>,
    incoming: Rc<Incoming>,
}

impl State {
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
}

/// A single-threaded task pool for polling futures to completion.
///
/// This executor allows you to multiplex any number of tasks onto a single
/// thread. It's appropriate to poll strictly I/O-bound futures that do very
/// little work in between I/O actions.
///
/// To get a handle to the pool that implements
/// [`Spawn`](futures_core::task::Spawn), use the
/// [`spawner()`](LocalPool::spawner) method. Because the executor is
/// single-threaded, it supports a special form of task spawning for non-`Send`
/// futures, via [`spawn_local_obj`](futures_core::task::LocalSpawn::spawn_local_obj).
#[derive(Debug)]
pub struct LocalPool<P = ParkThread> {
    park: P,
    state: State,
}

/// A handle to a [`LocalPool`](LocalPool) that implements
/// [`Spawn`](futures_core::task::Spawn).
#[derive(Clone, Debug)]
pub struct LocalSpawner {
    incoming: Weak<Incoming>,
}

type Incoming = RefCell<Vec<LocalFutureObj<'static, ()>>>;

impl LocalPool<ParkThread> {
    /// Create a new, empty pool of tasks.
    pub fn new() -> Self
    {
        Self::new_with_park(ParkThread::default())
    }
}

impl<P> LocalPool<P>
where
    P: Park,
    P::Error: std::fmt::Debug,
{
    /// Create a new, empty pool of tasks.
    pub fn new_with_park(park: P) -> Self {
        LocalPool {
            park,
            state: State {
                pool: FuturesUnordered::new(),
                incoming: Default::default(),
            },
        }
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
        LocalSpawner {
            incoming: Rc::downgrade(&self.state.incoming)
        }
    }

    /// Run all tasks in the pool to completion.
    ///
    /// The given spawner, `spawn`, is used as the default spawner for any
    /// *newly*-spawned tasks. You can route these additional tasks back into
    /// the `LocalPool` by using its spawner handle:
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
        self.run_executor(|state, cx| state.poll_pool(cx))
    }

    /// Runs all the tasks in the pool until the given future completes.
    ///
    /// The given spawner, `spawn`, is used as the default spawner for any
    /// *newly*-spawned tasks. You can route these additional tasks back into
    /// the `LocalPool` by using its spawner handle:
    ///
    /// ```
    /// use futures::executor::LocalPool;
    ///
    /// let mut pool = LocalPool::new();
    /// # let my_app  = async {};
    ///
    /// // run tasks in the pool until `my_app` completes, by default spawning
    /// // further tasks back onto the pool
    /// pool.run_until(my_app);
    /// ```
    ///
    /// The function will block the calling thread *only* until the future `f`
    /// completes; there may still be incomplete tasks in the pool, which will
    /// be inert after the call completes, but can continue with further use of
    /// one of the pool's run or poll methods. While the function is running,
    /// however, all tasks in the pool will try to make progress.
    pub fn run_until<F: Future>(&mut self, future: F) -> F::Output {
        pin_mut!(future);

        self.run_executor(|state, cx| {
            {
                // if our main task is done, so are we
                let result = future.as_mut().poll(cx);
                if let Poll::Ready(output) = result {
                    return Poll::Ready(output);
                }
            }

            let _ = state.poll_pool(cx);
            Poll::Pending
        })
    }

    /// Runs all tasks and returns after completing one future or until no more progress
    /// can be made. Returns `true` if one future was completed, `false` otherwise.
    ///
    /// ```
    /// use futures::executor::LocalPool;
    /// use futures::task::LocalSpawnExt;
    /// use futures::future::{ready, pending};
    ///
    /// let mut pool = LocalPool::new();
    /// let mut spawner = pool.spawner();
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
        let mut enter = enter()
            .expect("cannot execute `LocalPool` executor from within \
                    another executor");

        {
            // first round
            let waker = self.park.waker();
            let mut cx = Context::from_waker(&waker);

            let result = self.state.poll_pool_once(&mut cx);

            // return if we really have executed a future
            if let Poll::Ready(Some(_)) = result {
                return true;
            }
        }

        self.park.park(&mut enter, ParkDuration::Poll).expect("park failed");

        let waker = self.park.waker();
        let mut cx = Context::from_waker(&waker);

        let result = self.state.poll_pool_once(&mut cx);

        // return whether we really have executed a future
        match result {
            Poll::Ready(Some(_)) => true,
            _ => false
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
    /// let mut spawner = pool.spawner();
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
        let mut enter = enter()
            .expect("cannot execute `LocalPool` executor from within \
                    another executor");

        {
            // first round
            let waker = self.park.waker();
            let mut cx = Context::from_waker(&waker);

            // ignore first result, we need to run park anyway and try again
            let _ = self.state.poll_pool_once(&mut cx);
        }

        loop {
            self.park.park(&mut enter, ParkDuration::Poll).expect("park failed");

            let waker = self.park.waker();
            let mut cx = Context::from_waker(&waker);

            let result = self.state.poll_pool_once(&mut cx);

            // if there are no more ready futures exit
            match result {
                Poll::Pending | Poll::Ready(None) => return,
                _ => continue
            }
        }
    }

    // Set up and run a basic single-threaded spawner loop, invoking `f` on each
    // turn.
    fn run_executor<T, F: FnMut(&mut State, &mut Context<'_>) -> Poll<T>>(&mut self, mut f: F) -> T {
        let mut enter = enter()
            .expect("cannot execute `LocalPool` executor from within \
                    another executor");

        loop {
            let waker = self.park.waker();
            let mut cx = Context::from_waker(&waker);

            if let Poll::Ready(t) = f(&mut self.state, &mut cx) {
                return t;
            }
            self.park.park(&mut enter, ParkDuration::Block).expect("park failed");
        }
    }
}

impl<P> Default for LocalPool<P>
where
    P: Default + Park,
    P::Error: std::fmt::Debug,
{
    fn default() -> Self {
        Self::new_with_park(P::default())
    }
}

/// Run a future to completion on the current thread.
///
/// This function will block the caller until the given future has completed.
///
/// Use a [`LocalPool`](LocalPool) if you need finer-grained control over
/// spawned tasks.
pub fn block_on<F: Future>(f: F) -> F::Output {
    pin_mut!(f);

    let mut enter = enter()
        .expect("cannot execute `block_on` executor from within \
                another executor");

    let mut park = ParkThread::new();
    let waker = park.waker().clone();
    let mut cx = Context::from_waker(&waker);

    loop {
        if let Poll::Ready(t) = f.as_mut().poll(&mut cx) {
            return t;
        }
        park.park(&mut enter, ParkDuration::Block).unwrap_or_else(|i| match i {});
    }
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
pub struct BlockingStream<S: Stream + Unpin> { stream: S }

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
        LocalPool::<ParkThread>::new().run_until(self.stream.next())
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.stream.size_hint()
    }
}

impl Spawn for LocalSpawner {
    fn spawn_obj(
        &mut self,
        future: FutureObj<'static, ()>,
    ) -> Result<(), SpawnError> {
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
    fn spawn_local_obj(
        &mut self,
        future: LocalFutureObj<'static, ()>,
    ) -> Result<(), SpawnError> {
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
