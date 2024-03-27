use crate::{FutureObj, LocalFutureObj};
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
    fn spawn_obj(&self, future: FutureObj<'static, ()>) -> Result<(), SpawnError>;

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

/// The `LocalSpawn` is similar to [`Spawn`], but allows spawning futures
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
    fn spawn_local_obj(&self, future: LocalFutureObj<'static, ()>) -> Result<(), SpawnError>;

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

/// The `BoundLocalSpawn` is similar to [`LocalSpawn`], but allows spawning
/// futures that don't implement `Send` and have a lifetime that only needs to
/// exceed that of the associated executor.
pub trait BoundLocalSpawn<'a> {
    /// Spawns a future that will be run to completion or until the executor is
    /// dropped.
    ///
    /// # Errors
    ///
    /// The executor may be unable to spawn tasks. Spawn errors should
    /// represent relatively rare scenarios, such as the executor
    /// having been shut down so that it is no longer able to accept
    /// tasks.
    fn spawn_bound_local_obj(&self, future: LocalFutureObj<'a, ()>) -> Result<(), SpawnError>;
}

/// An error that occurred during spawning.
pub struct SpawnError {
    _priv: (),
}

impl fmt::Debug for SpawnError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("SpawnError").field(&"shutdown").finish()
    }
}

impl fmt::Display for SpawnError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Executor is shutdown")
    }
}

#[cfg(feature = "std")]
impl std::error::Error for SpawnError {}

impl SpawnError {
    /// Spawning failed because the executor has been shut down.
    pub fn shutdown() -> Self {
        Self { _priv: () }
    }

    /// Check whether spawning failed to the executor being shut down.
    pub fn is_shutdown(&self) -> bool {
        true
    }
}

impl<T, Sp: ?Sized + Spawn> Spawn for T
where
    T: core::ops::Deref<Target = Sp>,
{
    fn spawn_obj(&self, future: FutureObj<'static, ()>) -> Result<(), SpawnError> {
        Sp::spawn_obj(self, future)
    }

    fn status(&self) -> Result<(), SpawnError> {
        Sp::status(self)
    }
}

impl<T, Sp: ?Sized + LocalSpawn> LocalSpawn for T
where
    T: core::ops::Deref<Target = Sp>,
{
    fn spawn_local_obj(&self, future: LocalFutureObj<'static, ()>) -> Result<(), SpawnError> {
        Sp::spawn_local_obj(self, future)
    }

    fn status_local(&self) -> Result<(), SpawnError> {
        Sp::status_local(self)
    }
}

impl<'a, T, Sp: ?Sized + BoundLocalSpawn<'a>> BoundLocalSpawn<'a> for T
where
    T: core::ops::Deref<Target = Sp>,
{
    fn spawn_bound_local_obj(&self, future: LocalFutureObj<'a, ()>) -> Result<(), SpawnError> {
        Sp::spawn_bound_local_obj(self, future)
    }
}
