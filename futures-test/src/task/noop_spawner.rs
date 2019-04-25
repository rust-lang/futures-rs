use futures_core::future::FutureObj;
use futures_core::task::{Spawn, SpawnError};

/// An implementation of [`Spawn`](futures_core::task::Spawn) that
/// discards spawned futures when used.
///
/// # Examples
///
/// ```
/// #![feature(async_await)]
/// use futures::task::SpawnExt;
/// use futures_test::task::NoopSpawner;
///
/// let mut spawner = NoopSpawner::new();
/// spawner.spawn(async { });
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
    ) -> Result<(), SpawnError> {
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
/// #![feature(async_await)]
/// use futures::task::SpawnExt;
/// use futures_test::task::noop_spawner_mut;
///
/// let spawner = noop_spawner_mut();
/// spawner.spawn(async { });
/// ```
pub fn noop_spawner_mut() -> &'static mut NoopSpawner {
    Box::leak(Box::new(NoopSpawner::new()))
}
