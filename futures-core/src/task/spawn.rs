use crate::future::{FutureObj, LocalFutureObj};
use core::fmt;

/// The `Spawn` trait allows for pushing futures onto an executor that will
/// run them to completion.
pub trait Spawn {
    /// Spawns a future that will be run to completion.
    ///
    /// # Errors
    ///
    /// The executor may be unable to spawn tasks. Spawn errors should
    /// represent relatively rare scenarios, such as the executor
    /// having been shut down so that it is no longer able to accept
    /// tasks.
    fn spawn_obj(&mut self, future: FutureObj<'static, ()>)
        -> Result<(), SpawnError>;

    /// Determines whether the executor is able to spawn new tasks.
    ///
    /// This method will return `Ok` when the executor is *likely*
    /// (but not guaranteed) to accept a subsequent spawn attempt.
    /// Likewise, an `Err` return means that `spawn` is likely, but
    /// not guaranteed, to yield an error.
    #[inline]
    fn status(&self) -> Result<(), SpawnError> {
        Ok(())
    }
}

/// The `LocalSpawn` is similar to `[Spawn]`, but allows spawning futures
/// that don't implement `Send`.
pub trait LocalSpawn {
    /// Spawns a future that will be run to completion.
    ///
    /// # Errors
    ///
    /// The executor may be unable to spawn tasks. Spawn errors should
    /// represent relatively rare scenarios, such as the executor
    /// having been shut down so that it is no longer able to accept
    /// tasks.
    fn spawn_local_obj(&mut self, future: LocalFutureObj<'static, ()>)
        -> Result<(), SpawnError>;

    /// Determines whether the executor is able to spawn new tasks.
    ///
    /// This method will return `Ok` when the executor is *likely*
    /// (but not guaranteed) to accept a subsequent spawn attempt.
    /// Likewise, an `Err` return means that `spawn` is likely, but
    /// not guaranteed, to yield an error.
    #[inline]
    fn status_local(&self) -> Result<(), SpawnError> {
        Ok(())
    }
}

/// An error that occurred during spawning.
pub struct SpawnError {
    _hidden: (),
}

impl fmt::Debug for SpawnError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("SpanError")
            .field(&"shutdown")
            .finish()
    }
}

impl SpawnError {
    /// Spawning failed because the executor has been shut down.
    pub fn shutdown() -> Self {
        Self { _hidden: () }
    }

    /// Check whether spawning failed to the executor being shut down.
    pub fn is_shutdown(&self) -> bool {
        true
    }
}
