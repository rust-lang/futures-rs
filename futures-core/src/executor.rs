//! Executors.

if_std! {
    use std::boxed::Box;
    use Future;
    use never::Never;

    /// A task executor.
    ///
    /// A *task* is a `()`-producing future that runs at the top level, and will
    /// be `poll`ed until completion. It's also the unit at which wake-up
    /// notifications occur. Executors, such as thread pools, allow tasks to be
    /// spawned and are responsible for putting tasks onto ready queues when
    /// they are woken up, and polling them when they are ready.
    pub trait Executor {
        /// Spawn the given task, polling it until completion.
        ///
        /// Tasks must be infallible, as the type suggests; it is the
        /// client's reponsibility to route any errors elsewhere via a channel
        /// or some other means of communication.
        ///
        /// # Errors
        ///
        /// The executor may be unable to spawn tasks, either because it has
        /// been shut down or is resource-constrained.
        fn spawn(&mut self, f: Box<Future<Item = (), Error = Never> + Send>) -> Result<(), SpawnError>;

        /// Determine whether the executor is able to spawn new tasks.
        ///
        /// # Returns
        ///
        /// An `Ok` return means the executor is *likely* (but not guaranteed)
        /// to accept a subsequent spawn attempt. Likewise, an `Err` return
        /// means that `spawn` is likely, but not guaranteed, to yield an error.
        fn status(&self) -> Result<(), SpawnError> {
            Ok(())
        }

        // TODO: downcasting hooks
    }

    /// Provides the reason that an executor was unable to spawn.
    #[derive(Debug)]
    pub struct SpawnError {
        _a: ()
    }

    impl SpawnError {
        /// Spawning is failing because the executor has been shut down.
        pub fn shutdown() -> SpawnError {
            SpawnError { _a: () }
        }

        /// Check whether this error is the `shutdown` error.
        pub fn is_shutdown() -> bool {
            true
        }
    }
}

#[cfg(not(feature = "std"))]
pub(crate) trait Executor {}
