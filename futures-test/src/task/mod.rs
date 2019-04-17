//! Task related testing utilities.
//!
//! This module provides utilities for creating test
//! [`Context`](futures_core::task::Context)s,
//! [`Waker`](futures_core::task::Waker)s and
//! [`Spawn`](futures_core::task::Spawn) implementations.
//!
//! Test contexts:
//! - [`noop_context`] creates a context that ignores calls to
//!   [`cx.waker().wake_by_ref()`](futures_core::task::Waker).
//! - [`panic_context`] creates a context that panics when
//!   [`cx.waker().wake_by_ref()`](futures_core::task::Waker) is called.
//!
//! Test wakers:
//! - [`noop_waker`] creates a waker that ignores calls to
//!   [`wake`](futures_core::task::Waker).
//! - [`panic_waker`] creates a waker that panics when
//!   [`wake`](futures_core::task::Waker) is called.
//! - [`new_count_waker`] creates a waker that increments a counter whenever
//!   [`wake`](futures_core::task::Waker) is called.
//!
//! Test spawners:
//! - [`NoopSpawner`] ignores calls to
//!   [`spawn`](futures_core::task::Spawn::spawn)
//! - [`PanicSpawner`] panics if [`spawn`](futures_core::task::Spawn::spawn) is
//!   called.
//! - [`RecordSpawner`] records the spawned futures.
//!
//! For convenience there additionally exist various functions that directly
//! return waker/spawner references: [`noop_waker_ref`],
//! [`panic_waker_ref`], [`noop_spawner_mut`] and [`panic_spawner_mut`].

mod context;
pub use self::context::{noop_context, panic_context};

mod noop_spawner;
pub use self::noop_spawner::{noop_spawner_mut, NoopSpawner};

pub use futures_util::task::{noop_waker, noop_waker_ref};

mod panic_spawner;
pub use self::panic_spawner::{panic_spawner_mut, PanicSpawner};

mod panic_waker;
pub use self::panic_waker::{panic_waker, panic_waker_ref};

mod record_spawner;
pub use self::record_spawner::RecordSpawner;

mod wake_counter;
pub use self::wake_counter::{AwokenCount, new_count_waker};
