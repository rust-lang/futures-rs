#![feature(async_await, await_macro)]
#![cfg_attr(all(feature = "async-stream", feature = "nightly"), feature(generators, stmt_expr_attributes, proc_macro_hygiene))]

#[cfg(all(feature = "async-stream", feature = "nightly"))]
mod async_stream;
