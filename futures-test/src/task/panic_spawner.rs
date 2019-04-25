use futures_core::future::FutureObj;
use futures_core::task::{Spawn, SpawnError};

/// An implementation of [`Spawn`](futures_core::task::Spawn) that panics
/// when used.
///
/// # Examples
///
/// ```should_panic
/// #![feature(async_await)]
/// use futures::task::SpawnExt;
/// use futures_test::task::PanicSpawner;
///
/// let mut spawn = PanicSpawner::new();
/// spawn.spawn(async { }); // Will panic
/// ```
#[derive(Debug)]
pub struct PanicSpawner {
    _reserved: (),
}

impl PanicSpawner {
    /// Create a new instance
    pub fn new() -> Self {
        Self { _reserved: () }
    }
}

impl Spawn for PanicSpawner {
    fn spawn_obj(
        &mut self,
        _future: FutureObj<'static, ()>,
    ) -> Result<(), SpawnError> {
        panic!("should not spawn")
    }
}

impl Default for PanicSpawner {
    fn default() -> Self {
        Self::new()
    }
}

/// Get a reference to a singleton instance of [`PanicSpawner`].
///
/// # Examples
///
/// ```should_panic
/// #![feature(async_await)]
/// use futures::task::SpawnExt;
/// use futures_test::task::panic_spawner_mut;
///
/// let spawner = panic_spawner_mut();
/// spawner.spawn(async { }); // Will panic
/// ```
pub fn panic_spawner_mut() -> &'static mut PanicSpawner {
    Box::leak(Box::new(PanicSpawner::new()))
}
