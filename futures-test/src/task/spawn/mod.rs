//! Implementations of [`Spawn`][futures_core::task::Spawn] with various
//! behaviour for test purposes.

mod noop;
pub use self::noop::Noop;

mod panic;
pub use self::panic::Panic;

mod record;
pub use self::record::Record;
