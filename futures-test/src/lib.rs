//! Utilities to make testing [`Future`s][futures_core::Future] easier

#![feature(
    arbitrary_self_types,
    async_await,
    await_macro,
    futures_api,
    pin,
)]
#![warn(missing_docs, missing_debug_implementations)]
#![deny(bare_trait_objects)]
#![doc(
    html_root_url = "https://rust-lang-nursery.github.io/futures-doc/0.3.0-alpha.3/futures_test"
)]

#[doc(hidden)]
pub use std as std_reexport;

#[doc(hidden)]
pub extern crate futures_core as futures_core_reexport;

#[macro_use]
extern crate pin_utils;

#[macro_use]
#[doc(hidden)]
pub mod assert;

pub mod task;

pub mod future;
