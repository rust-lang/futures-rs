//! Task related utilities.
//!
//! In the majority of use cases you can use the functions exported below to
//! create a [`Context`](futures_core::task::Context) appropriate to use in your
//! tests.
//!
//! For more complex test cases you can take a `Context` from one of these
//! functions and then use the
//! [`Context::with_waker`](futures_core::task::Context::with_waker) and
//! [`Context::with_spawner`](futures_core::task::Context::with_spawner)
//! methods to change the implementations used. See the examples on
//! the provided implementations in [`wake`] and
//! [`spawn`] for more details.

mod context;
pub use self::context::{no_spawn_context, noop_context, panic_context};

mod noop_spawner;
pub use self::noop_spawner::{noop_spawner_mut, NoopSpawner};

mod noop_waker;
pub use self::noop_waker::{noop_local_waker, noop_local_waker_ref, NoopWake};

mod panic_spawner;
pub use self::panic_spawner::{panic_spawner_mut, PanicSpawner};

mod panic_waker;
pub use self::panic_waker::{panic_local_waker, panic_local_waker_ref, PanicWake};

mod record_spawner;
pub use self::record_spawner::RecordSpawner;

mod wake_counter;
pub use self::wake_counter::WakeCounter;
