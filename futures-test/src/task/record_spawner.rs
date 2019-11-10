use futures_task::{FutureObj, Spawn, SpawnError};
use std::cell::{Ref, RefCell};

/// An implementation of [`Spawn`](futures_task::Spawn) that records
/// any [`Future`](futures_core::future::Future)s spawned on it.
///
/// # Examples
///
/// ```
/// use futures::task::SpawnExt;
/// use futures_test::task::RecordSpawner;
///
/// let recorder = RecordSpawner::new();
/// recorder.spawn(async { }).unwrap();
/// assert_eq!(recorder.spawned().len(), 1);
/// ```
#[derive(Debug, Default)]
pub struct RecordSpawner {
    spawned: RefCell<Vec<FutureObj<'static, ()>>>,
}

impl RecordSpawner {
    /// Create a new instance
    pub fn new() -> Self {
        Default::default()
    }

    /// Inspect any futures that were spawned onto this [`Spawn`].
    pub fn spawned(&self) -> Ref<'_, Vec<FutureObj<'static, ()>>> {
        self.spawned.borrow()
    }
}

impl Spawn for RecordSpawner {
    fn spawn_obj(&self, future: FutureObj<'static, ()>) -> Result<(), SpawnError> {
        self.spawned.borrow_mut().push(future);
        Ok(())
    }
}
