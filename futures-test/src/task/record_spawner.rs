use futures_core::future::FutureObj;
use futures_core::task::{Spawn, SpawnObjError};

/// An implementation of [`Spawn`](futures_core::task::Spawn) that records
/// any [`Future`](futures_core::future::Future)s spawned on it.
///
/// # Examples
///
/// ```
/// #![feature(async_await, futures_api)]
/// use futures::task::SpawnExt;
/// use futures_test::task::{panic_context, RecordSpawner};
///
/// let mut recorder = RecordSpawner::new();
///
/// {
///     let mut lw = panic_context();
///     let lw = &mut lw.with_spawner(&mut recorder);
///     lw.spawner().spawn(async { });
/// }
///
/// assert_eq!(recorder.spawned().len(), 1);
/// ```
#[derive(Debug)]
pub struct RecordSpawner {
    spawned: Vec<FutureObj<'static, ()>>,
}

impl RecordSpawner {
    /// Create a new instance
    pub fn new() -> Self {
        Self {
            spawned: Vec::new(),
        }
    }

    /// Inspect any futures that were spawned onto this [`Spawn`].
    pub fn spawned(&self) -> &[FutureObj<'static, ()>] {
        &self.spawned
    }
}

impl Spawn for RecordSpawner {
    fn spawn_obj(
        &mut self,
        future: FutureObj<'static, ()>,
    ) -> Result<(), SpawnObjError> {
        self.spawned.push(future);
        Ok(())
    }
}

impl Default for RecordSpawner {
    fn default() -> Self {
        Self::new()
    }
}
