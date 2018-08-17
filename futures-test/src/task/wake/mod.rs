//! Implementations of [`Wake`](futures_core::task::Wake) with various behaviour
//! for test purposes.

mod counter;
pub use self::counter::Counter;

mod noop;
pub use self::noop::{noop_local_waker, noop_local_waker_ref, Noop};

mod panic;
pub use self::panic::{panic_local_waker, panic_local_waker_ref, Panic};
