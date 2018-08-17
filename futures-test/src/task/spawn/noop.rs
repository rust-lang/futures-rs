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
/// use futures_test::task::{panic_context, spawn};
///
/// let mut cx = panic_context();
/// let mut spawn = spawn::Noop::new();
/// let cx = &mut cx.with_spawner(&mut spawn);
///
/// cx.spawner().spawn(async { });
/// ```
#[derive(Debug)]
pub struct Noop {
    _reserved: (),
}

impl Noop {
    /// Create a new instance
    pub fn new() -> Self {
        Self { _reserved: () }
    }
}

impl Spawn for Noop {
    fn spawn_obj(
        &mut self,
        _future: FutureObj<'static, ()>,
    ) -> Result<(), SpawnObjError> {
        Ok(())
    }
}

impl Default for Noop {
    fn default() -> Self {
        Self::new()
    }
}

/// Get a thread local reference to a singleton instance of [`Noop`].
pub fn noop_mut() -> &'static mut Noop {
    static mut INSTANCE: Noop = Noop { _reserved: () };
    // Safety: This is safe because `Noop` is a ZST
    unsafe { &mut INSTANCE }
}
