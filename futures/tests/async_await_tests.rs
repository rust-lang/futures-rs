#![cfg_attr(feature = "nightly", feature(proc_macro, proc_macro_non_items, generators, pin))]

extern crate futures;

#[cfg(feature = "nightly")]
mod async_await;
