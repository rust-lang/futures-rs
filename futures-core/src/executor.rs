//! Executors.

use task::TaskObj;

/// A task executor.
///
/// A *task* is a `()`-producing future that runs at the top level, and will
/// be `poll`ed until completion. It's also the unit at which wake-up
/// notifications occur. Executors, such as thread pools, allow tasks to be
/// spawned and are responsible for putting tasks onto ready queues when
/// they are woken up, and polling them when they are ready.
pub trait Executor {
    /// Spawn the given task object, polling it until completion.
    ///
    /// # Errors
    ///
    /// The executor may be unable to spawn tasks, either because it has
    /// been shut down or is resource-constrained.
    fn spawn_obj(&mut self, task: TaskObj) -> Result<(), SpawnObjError>;

    /// Determine whether the executor is able to spawn new tasks.
    ///
    /// # Returns
    ///
    /// An `Ok` return means the executor is *likely* (but not guaranteed)
    /// to accept a subsequent spawn attempt. Likewise, an `Err` return
    /// means that `spawn` is likely, but not guaranteed, to yield an error.
    fn status(&self) -> Result<(), SpawnErrorKind> {
        Ok(())
    }

    // TODO: downcasting hooks
}

/// Provides the reason that an executor was unable to spawn.
#[derive(Debug)]
pub struct SpawnErrorKind {
    _a: ()
}

impl SpawnErrorKind {
    /// Spawning is failing because the executor has been shut down.
    pub fn shutdown() -> SpawnErrorKind {
        SpawnErrorKind { _a: () }
    }

    /// Check whether this error is the `shutdown` error.
    pub fn is_shutdown() -> bool {
        true
    }
}

/// The result of a failed spawn
#[derive(Debug)]
pub struct SpawnObjError {
    /// The kind of error
    pub kind: SpawnErrorKind,

    /// The task for which spawning was attempted
    pub task: TaskObj,
}
