//! Asynchronous channels.
//!
//! This crate provides channels that can be used to communicate between
//! asynchronous tasks.

#![feature(futures_api)]

#![cfg_attr(not(feature = "std"), no_std)]

#![warn(missing_docs, missing_debug_implementations)]
#![deny(bare_trait_objects)]

#![doc(html_root_url = "https://rust-lang-nursery.github.io/futures-api-docs/0.3.0-alpha.11/futures_intrusive")]

mod intrusive_list;

pub mod sync;
pub mod channel;