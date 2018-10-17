use crate::task::{ArcWake, Waker};
use futures_core::task::Context;
use slab::Slab;
use std::fmt;
use std::sync::{Arc, Mutex};

/// Type for keys into the waker set
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct WakerKey(usize);

impl WakerKey {
    /// null value for a waker key
    pub const NULL: WakerKey = WakerKey(usize::max_value());
}

impl fmt::Debug for WakerKey {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(fmt)
    }
}

/// An object that contains a set of wakers that can be notified
/// together.
///
/// Useful for implementing shared futures and streams
pub struct WakerSet {
    wakers: Mutex<Option<Slab<Option<Waker>>>>,
}

impl WakerSet {
    pub fn new() -> Self {
        Self {
            wakers: Mutex::new(Some(Slab::new())),
        }
    }

    /// Registers the current task to receive a wakeup when we are awoken.
    pub fn record_waker(&self, key: &mut WakerKey, cx: &mut Context<'_>) {
        let mut wakers_guard = self.wakers.lock().unwrap();

        let wakers = match wakers_guard.as_mut() {
            Some(wakers) => wakers,
            None => return,
        };
        let new_waker = cx.waker();

        if *key == WakerKey::NULL {
            *key = WakerKey(wakers.insert(Some(new_waker.clone())));
        } else {
            match wakers[key.0] {
                Some(ref old_waker) if new_waker.will_wake(old_waker) => {}
                // Could use clone_from here, but Waker doesn't specialize it.
                ref mut slot => *slot = Some(new_waker.clone()),
            }
        }
        debug_assert!(*key != WakerKey::NULL);
    }

    pub fn unregister(&self, key: WakerKey) {
        if key == WakerKey::NULL {
            return;
        }
        if let Ok(mut wakers) = self.wakers.lock() {
            if let Some(wakers) = wakers.as_mut() {
                wakers.remove(key.0);
            }
        }
    }

    /// Wake all registered wakers
    pub fn wake_all(&self) {
        let wakers = self.wakers.lock().unwrap();

        if let Some(wakers) = wakers.as_mut() {
            for (_, opt_waker) in wakers {
                if let Some(waker) = opt_waker.take() {
                    waker.wake()
                }
            }
        }
    }

    /// Wake all registered wakers and stop notifying
    ///
    /// After waking existing wakers, this drops the underlying buffer, and will
    /// no longer notify more wakers.
    pub fn wake_and_finish(&self) {
        let mut wakers_guard = self.wakers.lock().unwrap();
        if let Some(mut wakers) = wakers_guard.take() {
            for waker in wakers.drain().flatten() {
                waker.wake();
            }
        }
    }
}

impl ArcWake for WakerSet {
    fn wake_by_ref(arc_self: &Arc<Self>) {
        arc_self.wake_all();
    }
}
