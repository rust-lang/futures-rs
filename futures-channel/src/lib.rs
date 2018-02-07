//! Asynchronous channels
//!
//! This crate provides channels which can be used to communicate between futures.

#![deny(missing_docs, missing_debug_implementations)]
#![doc(html_root_url = "https://docs.rs/futures/0.2")]

extern crate futures_core;

use futures_core::{Future, Stream, Poll, Async};
use futures_core::task::{self, Task};

mod lock;
pub mod mpsc;
pub mod oneshot;
