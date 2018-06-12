#![cfg_attr(feature = "nightly", feature(proc_macro, generators, pin))]

extern crate futures;

#[cfg(feature = "nightly")]
mod async_await;
