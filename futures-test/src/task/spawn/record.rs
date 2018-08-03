use futures_core::future::FutureObj;
use futures_core::task::{Spawn, SpawnObjError};

/// An implementation of [`Spawn`][futures_core::task::Spawn] that records
/// any [`Future`][futures_core::future::Future]s spawned on it.
///
/// # Examples
///
/// ```
/// #![feature(async_await, futures_api)]
/// use futures::task::SpawnExt;
/// use futures_test::task::{panic_context, spawn};
///
/// let mut recorder = spawn::Record::new();
///
/// {
///     let mut cx = panic_context();
///     let cx = &mut cx.with_spawner(&mut recorder);
///     cx.spawner().spawn(async { });
/// }
///
/// assert_eq!(recorder.spawned().len(), 1);
/// ```
#[derive(Debug)]
pub struct Record {
    spawned: Vec<FutureObj<'static, ()>>,
}

impl Record {
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

impl Spawn for Record {
    fn spawn_obj(
        &mut self,
        future: FutureObj<'static, ()>,
    ) -> Result<(), SpawnObjError> {
        self.spawned.push(future);
        Ok(())
    }
}

impl Default for Record {
    fn default() -> Self {
        Self::new()
    }
}
