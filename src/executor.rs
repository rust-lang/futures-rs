//! Exeuctors
//!
//! This module contains tools for managing the raw execution of futures,
//! which is needed when building *executors* (places where futures can run).
// TODO: more dox

pub use task_impl::{Spawn, spawn, Unpark, Executor, Run};
