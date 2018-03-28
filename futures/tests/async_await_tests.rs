#![cfg_attr(feature = "nightly", feature(proc_macro, conservative_impl_trait, generators, pin))]

extern crate futures;

#[cfg(feature = "nightly")]
mod async_await;
