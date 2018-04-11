use core::fmt;
use executor::Executor;
use task::{Waker, LocalMap};

/// Information about the currently-running task.
///
/// Contexts are always tied to the stack, since they are set up specifically
/// when performing a single `poll` step on a task.
pub struct Context<'a> {
    waker: &'a Waker,
    pub(crate) map: &'a mut LocalMap,
    executor: Option<&'a mut Executor>,
}

impl<'a> Context<'a> {
    /// Create a new task context without the ability to `spawn`.
    ///
    /// This constructor should *only* be used for `no_std` contexts, where the
    /// standard `Executor` trait is not available.
    pub fn without_spawn(map: &'a mut LocalMap, waker: &'a Waker) -> Context<'a> {
        Context { waker, map, executor: None }
    }

    /// Get the [`Waker`](::task::Waker) associated with the current task.
    ///
    /// The waker can subsequently be used to wake up the task when some
    /// event of interest has happened.
    pub fn waker(&self) -> &Waker {
        self.waker
    }

    fn with_parts<'b, F, R>(&'b mut self, f: F) -> R
        where F: FnOnce(&'b Waker, &'b mut LocalMap, Option<&'b mut Executor>) -> R
    {
        // reborrow the executor
        let executor: Option<&'b mut Executor> = match self.executor {
            None => None,
            Some(ref mut e) => Some(&mut **e),
        };
        f(self.waker, self.map, executor)
    }

    /// Produce a context like the current one, but using the given waker
    /// instead.
    ///
    /// This advanced method is primarily used when building "internal
    /// schedulers" within a task, where you want to provide some customized
    /// wakeup logic.
    pub fn with_waker<'b>(&'b mut self, waker: &'b Waker) -> Context<'b> {
        self.with_parts(|_, map, executor| {
            Context { map, executor, waker }
        })
    }

    /// Produce a context like the current one, but using the given task locals
    /// instead.
    ///
    /// This advanced method is primarily used when building "internal
    /// schedulers" within a task.
    pub fn with_locals<'b>(&'b mut self, map: &'b mut LocalMap)
                           -> Context<'b>
    {
        self.with_parts(move |waker, _, executor| {
            Context { map, executor, waker }
        })
    }
}

if_std! {
    use std::boxed::Box;
    use Future;
    use never::Never;

    impl<'a> Context<'a> {
        /// Create a new task context.
        ///
        /// Task contexts are equipped with:
        ///
        /// - Task-local data
        /// - A means of waking the task
        /// - A means of spawning new tasks, i.e. an [executor]()
        pub fn new(map: &'a mut LocalMap, waker: &'a Waker, executor: &'a mut Executor) -> Context<'a> {
            Context { waker, map, executor: Some(executor) }
        }

        /// Get the default executor associated with this task, if any
        ///
        /// This method is useful primarily if you want to explicitly handle
        /// spawn failures.
        pub fn executor(&mut self) -> &mut Executor {
            self.executor
                .as_mut().map(|x| &mut **x)
                .expect("No default executor found: std-using futures contexts must provide an executor")
        }

        /// Spawn a future onto the default executor.
        ///
        /// # Panics
        ///
        /// This method will panic if the default executor is unable to spawn
        /// or does not exist.
        ///
        /// To handle executor errors, use [executor()](self::Context::executor)
        /// instead.
        pub fn spawn<F>(&mut self, f: F)
            where F: Future<Item = (), Error = Never> + 'static + Send
        {
            self.executor()
                .spawn(Box::new(f)).unwrap()
        }

        /// Produce a context like the current one, but using the given executor
        /// instead.
        ///
        /// This advanced method is primarily used when building "internal
        /// schedulers" within a task.
        pub fn with_executor<'b>(&'b mut self, executor: &'b mut Executor)
                            -> Context<'b>
        {
            self.with_parts(move |waker, map, _| {
                Context { map, executor: Some(executor), waker }
            })
        }
    }
}

impl<'a> fmt::Debug for Context<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Context")
            .finish()
    }
}
