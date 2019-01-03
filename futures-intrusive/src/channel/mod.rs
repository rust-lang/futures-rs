//! Asynchronous channels.
//!
//! This module provides various channels that can be used to communicate between
//! asynchronous tasks.

mod oneshot;

pub use self::oneshot::{
    LocalOneshotChannel, LocalOneshotReceiveFuture,
};

#[cfg(feature = "std")]
pub use self::oneshot::{
    OneshotChannel, OneshotReceiveFuture,
};
