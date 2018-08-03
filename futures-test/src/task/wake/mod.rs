//! Implementations of [`Wake`][futures_core::task::Wake] with various behaviour
//! for test purposes.

mod counter;
pub use self::counter::Counter;

mod noop;
pub use self::noop::Noop;

mod panic;
pub use self::panic::Panic;
