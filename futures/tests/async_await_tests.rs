#![cfg_attr(feature = "nightly", feature(use_extern_macros, proc_macro_non_items, generators, pin))]

extern crate futures;

#[cfg(feature = "nightly")]
mod async_await;
