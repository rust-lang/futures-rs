//! An unbounded map of futures.
//!
//! This module is only available when the `std` or `alloc` feature of this
//! library is activated, and it is activated by default.

use crate::task::AtomicWaker;
use alloc::sync::{Arc, Weak};
use core::cell::UnsafeCell;
use core::fmt::{self, Debug};
use core::iter::FromIterator;
use core::marker::PhantomData;
use core::mem;
use core::pin::Pin;
use core::ptr;
use core::sync::atomic::Ordering::{AcqRel, Acquire, Relaxed, Release, SeqCst};
use core::sync::atomic::{AtomicBool, AtomicPtr};
use futures_core::future::Future;
use futures_core::stream::{FusedStream, Stream};
use futures_core::task::{Context, Poll};
use futures_task::{FutureObj, LocalFutureObj, LocalSpawn, Spawn, SpawnError};
use std::collections::{HashMap, HashSet};

use super::FuturesUnordered;
