//! Futures 0.1 / 0.3 shims

#![allow(missing_debug_implementations)]

mod executor;
pub use self::executor::{Executor01CompatExt, Executor01Future, Executor01As03};

mod compat;
pub use self::compat::Compat;

mod compat01to03;
mod compat03to01;

mod future01ext;
pub use self::future01ext::Future01CompatExt;

mod stream01ext;
pub use self::stream01ext::Stream01CompatExt;

#[cfg(feature = "tokio-compat")]
mod tokio;
#[cfg(feature = "tokio-compat")]
pub use self::tokio::TokioDefaultSpawner;
