use std::mem;

use cell::AtomicCell;

/// A slot in memory intended to represent the communication channel between one
/// producer and one consumer.
///
/// Each slot contains space for a piece of data, `T`, and space for callbacks.
/// The callbacks can be run when the data is either full or empty.
pub struct Slot<T> {
    slot: AtomicCell<Option<T>>,
    callback: AtomicCell<Option<Box<FnBox<T>>>>,
}

/// Error value returned from erroneous calls to `try_produce`.
pub struct TryProduceError<T>(T);

/// Error value returned from erroneous calls to `try_consume`.
pub struct TryConsumeError(());

/// Error value returned from erroneous calls to `on_full`.
pub struct OnFullError(());

fn _is_send<T: Send>() {}
fn _is_sync<T: Send>() {}

fn _assert() {
    _is_send::<Slot<i32>>();
    _is_sync::<Slot<u32>>();
}

impl<T: 'static> Slot<T> {
    pub fn new(val: Option<T>) -> Slot<T> {
        Slot {
            slot: AtomicCell::new(val),
            callback: AtomicCell::new(None),
        }
    }

    pub fn try_produce(&self, t: T) -> Result<(), TryProduceError<T>> {
        let mut slot = match self.slot.borrow() {
            Some(borrow) => borrow,
            None => return Err(TryProduceError(t)),
        };
        match mem::replace(&mut *slot, Some(t)) {
            Some(t) => return Err(TryProduceError(t)),
            None => {}
        }
        drop(slot);
        let callback = self.callback.borrow().and_then(|mut cb| {
            mem::replace(&mut *cb, None)
        });
        if let Some(callback) = callback {
            callback.call_box(self);
        }
        Ok(())
    }

    pub fn on_full<F>(&self, f: F) -> Result<(), OnFullError>
        where F: FnOnce(&Slot<T>) + Send + 'static
    {
        let has_data = self.slot.borrow().is_some();
        if has_data {
            return Ok(f(self))
        }
        match self.callback.borrow() {
            Some(mut callback) => *callback = Some(Box::new(f)),
            None => return Ok(f(self)),
        }

        let has_data = self.slot.borrow().is_some();
        if has_data {
            let cb = self.callback.borrow().and_then(|mut cb| {
                mem::replace(&mut *cb, None)
            });
            if let Some(cb) = cb {
                cb.call_box(self);
            }
        }
        Ok(())
    }

    pub fn try_consume(&self) -> Result<T, TryConsumeError> {
        let mut slot = match self.slot.borrow() {
            Some(borrow) => borrow,
            None => return Err(TryConsumeError(())),
        };
        match mem::replace(&mut *slot, None) {
            Some(t) => Ok(t),
            None => Err(TryConsumeError(())),
        }
    }
}

trait FnBox<T: 'static>: Send + 'static {
    fn call_box(self: Box<Self>, other: &Slot<T>);
}

impl<T, F> FnBox<T> for F
    where F: FnOnce(&Slot<T>) + Send + 'static,
          T: 'static,
{
    fn call_box(self: Box<F>, other: &Slot<T>) {
        (*self)(other)
    }
}
