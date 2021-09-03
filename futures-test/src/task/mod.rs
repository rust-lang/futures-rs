// TODO: note that paths like futures_core::task::Context actually get redirected to core::task::Context
// in the rendered docs. Is this desirable? If so, should we change the paths here?
//
// Also, there is cross crate links in here. They are not going to work anytime soon. Do we put https links
// in here? to here: https://rust-lang.github.io/futures-api-docs? The problem is these have a
// version hardcoded in the url: 0.3.0-alpha.16 We could link to docs.rs, but currently that says:
// docs.rs failed to build futures-0.3.0-alpha.16 -> ok the reason seems to be that they are on
// 2019-04-17 which does still have futures-api unstable feature, so that should get solved.
//
//! Task related testing utilities.
//!
//! This module provides utilities for creating test
//! [`Context`](futures_core::task::Context)s,
//! [`Waker`](futures_core::task::Waker)s and
//! [`Spawn`](futures_task::Spawn) implementations.
//!
//! Test contexts:
//! - [`noop_context`](crate::task::noop_context) creates a context that ignores calls to
//!   [`cx.waker().wake_by_ref()`](futures_core::task::Waker).
//! - [`panic_context`](crate::task::panic_context) creates a context that panics when
//!   [`cx.waker().wake_by_ref()`](futures_core::task::Waker) is called.
//!
//! Test wakers:
//! - [`noop_waker`](crate::task::noop_waker) creates a waker that ignores calls to
//!   [`wake`](futures_core::task::Waker).
//! - [`panic_waker`](crate::task::panic_waker) creates a waker that panics when
//!   [`wake`](futures_core::task::Waker) is called.
//! - [`new_count_waker`](crate::task::new_count_waker) creates a waker that increments a counter whenever
//!   [`wake`](futures_core::task::Waker) is called.
//!
//! Test spawners:
//! - [`NoopSpawner`](crate::task::NoopSpawner) ignores calls to
//!   [`spawn`](futures_util::task::SpawnExt::spawn)
//! - [`PanicSpawner`](crate::task::PanicSpawner) panics if [`spawn`](futures_util::task::SpawnExt::spawn) is
//!   called.
//! - [`RecordSpawner`](crate::task::RecordSpawner) records the spawned futures.
//!
//! For convenience there additionally exist various functions that directly
//! return waker/spawner references: [`noop_waker_ref`](crate::task::noop_waker_ref),
//! [`panic_waker_ref`](crate::task::panic_waker_ref), [`noop_spawner_mut`](crate::task::noop_spawner_mut) and [`panic_spawner_mut`](crate::task::panic_spawner_mut).

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
pub use self::wake_counter::{new_count_waker, AwokenCount};
