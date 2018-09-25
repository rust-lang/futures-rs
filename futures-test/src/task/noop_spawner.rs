use futures_core::future::FutureObj;
use futures_core::task::{Spawn, SpawnObjError};

/// An implementation of [`Spawn`](futures_core::task::Spawn) that
/// discards spawned futures when used.
///
/// # Examples
///
/// ```
/// #![feature(async_await, futures_api)]
/// use futures::task::SpawnExt;
/// use futures_test::task::{panic_context, NoopSpawner};
///
/// let mut lw = panic_context();
/// let mut spawn = NoopSpawner::new();
/// let lw = &mut lw.with_spawner(&mut spawn);
///
/// lw.spawner().spawn(async { });
/// ```
#[derive(Debug)]
pub struct NoopSpawner {
    _reserved: (),
}

impl NoopSpawner {
    /// Create a new instance
    pub fn new() -> Self {
        Self { _reserved: () }
    }
}

impl Spawn for NoopSpawner {
    fn spawn_obj(
        &mut self,
        _future: FutureObj<'static, ()>,
    ) -> Result<(), SpawnObjError> {
        Ok(())
    }
}

impl Default for NoopSpawner {
    fn default() -> Self {
        Self::new()
    }
}

/// Get a reference to a singleton instance of [`NoopSpawner`].
///
/// # Examples
///
/// ```
/// #![feature(async_await, futures_api)]
/// use futures::task::{self, SpawnExt};
/// use futures_test::task::{noop_local_waker_ref, noop_spawner_mut};
///
/// let mut lw = task::Context::new(
///     noop_local_waker_ref(),
///     noop_spawner_mut(),
/// );
///
/// lw.spawner().spawn(async { });
/// ```
pub fn noop_spawner_mut() -> &'static mut NoopSpawner {
    Box::leak(Box::new(NoopSpawner::new()))
}
