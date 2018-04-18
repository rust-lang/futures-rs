use core::fmt;

use executor::Executor;
use task::{TaskObj, Waker};

/// Information about the currently-running task.
///
/// Contexts are always tied to the stack, since they are set up specifically
/// when performing a single `poll` step on a task.
pub struct Context<'a> {
    waker: &'a Waker,
    executor: &'a mut Executor,
}

impl<'a> Context<'a> {
    /// Create a new task context.
    ///
    /// Task contexts are equipped with:
    /// - A means of waking the task
    /// - A means of spawning new tasks, i.e. an [executor]()
    pub fn new<E>(waker: &'a Waker, executor: &'a mut E) -> Context<'a>
        where E: Executor
    {
        Context { waker, executor }
    }

    /// Get the default executor associated with this task.
    ///
    /// This method is useful primarily if you want to explicitly handle
    /// spawn failures.
    pub fn executor(&mut self) -> &mut Executor {
        self.executor
    }

    /// Produce a context like the current one, but using the given executor
    /// instead.
    ///
    /// This advanced method is primarily used when building "internal
    /// schedulers" within a task.
    pub fn with_executor<'b, E>(&'b mut self, executor: &'b mut E) -> Context<'b>
        where E: Executor
    {
        Context { waker: self.waker, executor }
    }

    /// Get the [`Waker`](::task::Waker) associated with the current task.
    ///
    /// The waker can subsequently be used to wake up the task when some
    /// event of interest has happened.
    pub fn waker(&self) -> &Waker {
        self.waker
    }

    /// Produce a context like the current one, but using the given waker
    /// instead.
    ///
    /// This advanced method is primarily used when building "internal
    /// schedulers" within a task, where you want to provide some customized
    /// wakeup logic.
    pub fn with_waker<'b>(&'b mut self, waker: &'b Waker) -> Context<'b> {
        Context { waker, executor: self.executor }
    }
}

impl<'a> fmt::Debug for Context<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Context")
            .finish()
    }
}

if_std! {
    use Future;

    impl<'a> Context<'a> {
        /// Spawn a future onto the default executor.
        ///
        /// # Panics
        ///
        /// This method will panic if the default executor is unable to spawn.
        ///
        /// To handle executor errors, use [executor()](self::Context::executor)
        /// instead.
        pub fn spawn<F>(&mut self, f: F) where F: Future<Output = ()> + 'static + Send {
            self.executor()
                .spawn_obj(TaskObj::new(f)).unwrap()
        }
    }
}
