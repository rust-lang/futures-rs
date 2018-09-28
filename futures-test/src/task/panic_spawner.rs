use futures_core::future::FutureObj;
use futures_core::task::{Spawn, SpawnError};

/// An implementation of [`Spawn`](futures_core::task::Spawn) that panics
/// when used.
///
/// # Examples
///
/// ```should_panic
/// #![feature(async_await, futures_api)]
/// use futures::task::SpawnExt;
/// use futures_test::task::{noop_context, PanicSpawner};
///
/// let mut lw = noop_context();
/// let mut spawn = PanicSpawner::new();
/// let lw = &mut lw.with_spawner(&mut spawn);
///
/// lw.spawner().spawn(async { }); // Will panic
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
/// #![feature(async_await, futures_api)]
/// use futures::task::{self, SpawnExt};
/// use futures_test::task::{panic_local_waker_ref, panic_spawner_mut};
///
/// let mut lw = task::Context::new(
///     panic_local_waker_ref(),
///     panic_spawner_mut(),
/// );
///
/// lw.spawner().spawn(async { }); // Will panic
/// ```
pub fn panic_spawner_mut() -> &'static mut PanicSpawner {
    Box::leak(Box::new(PanicSpawner::new()))
}
