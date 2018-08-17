use futures_core::future::FutureObj;
use futures_core::task::{Spawn, SpawnObjError};
use std::cell::UnsafeCell;
use std::marker::PhantomData;

/// An implementation of [`Spawn`](futures_core::task::Spawn) that panics
/// when used.
///
/// # Examples
///
/// ```should_panic
/// #![feature(async_await, futures_api)]
/// use futures::task::SpawnExt;
/// use futures_test::task::{noop_context, spawn};
///
/// let mut cx = noop_context();
/// let mut spawn = spawn::Panic::new();
/// let cx = &mut cx.with_spawner(&mut spawn);
///
/// cx.spawner().spawn(async { }); // Will panic
/// ```
#[derive(Debug)]
pub struct Panic {
    // Required because we unsafely create references to thread locals
    _not_sync: PhantomData<*const ()>,
}

impl Panic {
    /// Create a new instance
    pub fn new() -> Self {
        Self { _not_sync: PhantomData }
    }
}

impl Spawn for Panic {
    fn spawn_obj(
        &mut self,
        _future: FutureObj<'static, ()>,
    ) -> Result<(), SpawnObjError> {
        panic!("should not spawn")
    }
}

impl Default for Panic {
    fn default() -> Self {
        Self::new()
    }
}

/// Get a thread local reference to a singleton instance of [`Panic`].
pub fn panic_mut() -> &'static mut Panic {
    thread_local! {
        static INSTANCE: UnsafeCell<Panic> =
            UnsafeCell::new(Panic { _not_sync: PhantomData });
    }
    INSTANCE.with(|i| unsafe { &mut *i.get() })
}
