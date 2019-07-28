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

/// The `SharedSpawn` trait allows for pushing futures onto an executor that will
/// run them to completion.
///
/// # Relationship with `Spawn`
///
/// `SharedSpawn` accepts `&self`, whereas `Spawn` accepts `&mut self`.
/// Therefore `SharedSpawn` can be shared between multiple entities.
pub trait SharedSpawn: Spawn {
    /// Spawns a future that will be run to completion.
    ///
    /// # Errors
    ///
    /// The executor may be unable to spawn tasks. Spawn errors should
    /// represent relatively rare scenarios, such as the executor
    /// having been shut down so that it is no longer able to accept
    /// tasks.
    fn shared_spawn_obj(&self, future: FutureObj<'static, ()>)
        -> Result<(), SpawnError>;
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
/// The `SharedLocalSpawn` is similar to [`SharedSpawn`], but allows spawning futures
/// that don't implement `Send`.
pub trait SharedLocalSpawn: LocalSpawn {
    /// Spawns a future that will be run to completion.
    ///
    /// # Errors
    ///
    /// The executor may be unable to spawn tasks. Spawn errors should
    /// represent relatively rare scenarios, such as the executor
    /// having been shut down so that it is no longer able to accept
    /// tasks.
    fn shared_spawn_local_obj(&self, future: LocalFutureObj<'static, ()>)
        -> Result<(), SpawnError>;
}

/// An error that occurred during spawning.
pub struct SpawnError {
    _hidden: (),
}

impl fmt::Debug for SpawnError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("SpawnError")
            .field(&"shutdown")
            .finish()
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
        Self { _hidden: () }
    }

    /// Check whether spawning failed to the executor being shut down.
    pub fn is_shutdown(&self) -> bool {
        true
    }
}

impl<Sp: ?Sized + Spawn> Spawn for &mut Sp {
    fn spawn_obj(&mut self, future: FutureObj<'static, ()>)
    -> Result<(), SpawnError> {
        Sp::spawn_obj(self, future)
    }

    fn status(&self) -> Result<(), SpawnError> {
        Sp::status(self)
    }
}

impl<Sp: ?Sized + SharedSpawn> SharedSpawn for &mut Sp {
    fn shared_spawn_obj(&self, future: FutureObj<'static, ()>)
    -> Result<(), SpawnError> {
        Sp::shared_spawn_obj(self, future)
    }
}

impl<Sp: ?Sized + SharedSpawn> Spawn for &Sp {
    fn spawn_obj(&mut self, future: FutureObj<'static, ()>)
    -> Result<(), SpawnError> {
        Sp::shared_spawn_obj(self, future)
    }

    fn status(&self) -> Result<(), SpawnError> {
        Sp::status(self)
    }
}

impl<Sp: ?Sized + SharedSpawn> SharedSpawn for &Sp {
    fn shared_spawn_obj(&self, future: FutureObj<'static, ()>)
    -> Result<(), SpawnError> {
        Sp::shared_spawn_obj(self, future)
    }
}

impl<Sp: ?Sized + LocalSpawn> LocalSpawn for &mut Sp {
    fn spawn_local_obj(&mut self, future: LocalFutureObj<'static, ()>)
    -> Result<(), SpawnError> {
        Sp::spawn_local_obj(self, future)
    }

    fn status_local(&self) -> Result<(), SpawnError> {
        Sp::status_local(self)
    }
}

impl<Sp: ?Sized + SharedLocalSpawn> SharedLocalSpawn for &mut Sp {
    fn shared_spawn_local_obj(&self, future: LocalFutureObj<'static, ()>)
    -> Result<(), SpawnError> {
        Sp::shared_spawn_local_obj(self, future)
    }
}

impl<Sp: ?Sized + SharedLocalSpawn> LocalSpawn for &Sp {
    fn spawn_local_obj(&mut self, future: LocalFutureObj<'static, ()>)
    -> Result<(), SpawnError> {
        Sp::shared_spawn_local_obj(self, future)
    }

    fn status_local(&self) -> Result<(), SpawnError> {
        Sp::status_local(self)
    }
}

impl<Sp: ?Sized + SharedLocalSpawn> SharedLocalSpawn for &Sp {
    fn shared_spawn_local_obj(&self, future: LocalFutureObj<'static, ()>)
    -> Result<(), SpawnError> {
        Sp::shared_spawn_local_obj(self, future)
    }
}

#[cfg(feature = "alloc")]
mod if_alloc {
    use alloc::boxed::Box;
    use super::*;

    impl<Sp: ?Sized + Spawn> Spawn for Box<Sp> {
        fn spawn_obj(&mut self, future: FutureObj<'static, ()>)
        -> Result<(), SpawnError> {
            (**self).spawn_obj(future)
        }

        fn status(&self) -> Result<(), SpawnError> {
            (**self).status()
        }
    }

    impl<Sp: ?Sized + SharedSpawn> SharedSpawn for Box<Sp> {
        fn shared_spawn_obj(&self, future: FutureObj<'static, ()>)
        -> Result<(), SpawnError> {
            (**self).shared_spawn_obj(future)
        }
    }

    impl<Sp: ?Sized + LocalSpawn> LocalSpawn for Box<Sp> {
        fn spawn_local_obj(&mut self, future: LocalFutureObj<'static, ()>)
        -> Result<(), SpawnError> {
            (**self).spawn_local_obj(future)
        }

        fn status_local(&self) -> Result<(), SpawnError> {
            (**self).status_local()
        }
    }

    impl<Sp: ?Sized + SharedLocalSpawn> SharedLocalSpawn for Box<Sp> {
        fn shared_spawn_local_obj(&self, future: LocalFutureObj<'static, ()>)
        -> Result<(), SpawnError> {
            (**self).shared_spawn_local_obj(future)
        }
    }
}
