//! Task related utilities.
//!
//! In the majority of use cases you can use the functions exported below to
//! create a [`Context`][futures_core::task::Context] appropriate to use in your
//! tests.
//!
//! For more complex test cases you can take a `Context` from one of these
//! functions and then use the
//! [`Context::with_waker`][futures_core::task::Context::with_waker] and
//! [`Context::with_spawner`][futures_core::task::Context::with_spawner]
//! methods to change the implementations used. See the examples on
//! the provided implementations in [`wake`] and
//! [`spawn`] for more details.

mod context;

pub mod spawn;
pub mod wake;

pub use self::context::{no_spawn_context, noop_context, panic_context};
