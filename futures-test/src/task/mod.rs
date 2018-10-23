//! Task related testing utilities.
//!
//! This module provides utilities for creating test
//! [`LocalWaker`](futures_core::task::LocalWaker)s and
//! [`Spawn`](futures_core::task::Spawn) implementations.
//!
//! Test wakers:
//! - [`noop_local_waker`] creates a waker that ignores calls to
//!   [`wake`](futures_core::task::LocalWaker).
//! - [`panic_local_waker`] creates a waker that panics when
//!   [`wake`](futures_core::task::LocalWaker) is called.
//! - [`WakeCounter::local_waker`] creates a waker that increments
//!   a counter whenever [`wake`](futures_core::task::LocalWaker) is called.
//!
//! Test spawners:
//! - [`NoopSpawner`] ignores calls to
//!   [`spawn`](futures_core::task::Spawn::spawn)
//! - [`PanicSpawner`] panics if [`spawn`](futures_core::task::Spawn::spawn) is
//!   called.
//! - [`RecordSpawner`] records the spawned futures.
//!
//! For convenience there additionally exist various functions that directly
//! return waker/spawner references: [`noop_local_waker_ref`],
//! [`panic_local_waker_ref`], [`noop_spawner_mut`] and [`panic_spawner_mut`].

mod noop_spawner;
pub use self::noop_spawner::{noop_spawner_mut, NoopSpawner};

pub use futures_util::task::{noop_local_waker, noop_local_waker_ref};

mod panic_spawner;
pub use self::panic_spawner::{panic_spawner_mut, PanicSpawner};

mod panic_waker;
pub use self::panic_waker::{panic_local_waker, panic_local_waker_ref, PanicWake};

mod record_spawner;
pub use self::record_spawner::RecordSpawner;

mod wake_counter;
pub use self::wake_counter::WakeCounter;
