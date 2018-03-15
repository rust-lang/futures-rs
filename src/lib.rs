//! Runtime support for the async/await syntax for futures.
//!
//! This crate serves as a masquerade over the `futures` crate itself,
//! reexporting all of its contents. It's intended that you'll do:
//!
//! ```
//! extern crate futures_await as futures;
//! ```
//!
//! This crate adds a `prelude` module which contains various traits as well as
//! the `async` and `await` macros you'll likely want to use.
//!
//! See the crates's README for more information about usage.

#![feature(conservative_impl_trait)]
#![feature(generator_trait)]
#![feature(use_extern_macros)]
#![feature(on_unimplemented)]
#![feature(arbitrary_self_types)]
#![feature(optin_builtin_traits)]

extern crate futures_await_async_macro as async_macro;
extern crate futures_await_await_macro as await_macro;
extern crate futures;

pub use futures::*;

pub mod prelude {
    pub use futures::prelude::*;
    pub use async_macro::{async, async_move, async_stream, async_stream_move, async_block, async_stream_block};
    pub use await_macro::{await, stream_yield, await_item};
}

/// A hidden module that's the "runtime support" for the async/await syntax.
///
/// The `async` attribute and the `await` macro both assume that they can find
/// this module and use its contents. All of their dependencies are defined or
/// reexported here in one way shape or form.
///
/// This module has absolutely not stability at all. All contents may change at
/// any time without notice. Do not use this module in your code if you wish
/// your code to be stable.
#[doc(hidden)]
pub mod __rt;
