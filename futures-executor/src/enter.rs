use std::prelude::v1::*;
use std::cell::Cell;
use std::fmt;

thread_local!(static ENTERED: Cell<bool> = Cell::new(false));

/// Represents an executor context.
///
/// For more details, see [`enter` documentation](fn.enter.html)
pub struct Enter {
    _a: ()
}

/// An error returned by `enter` if an execution scope has already been
/// entered.
#[derive(Debug)]
pub struct EnterError {
    _a: (),
}

/// Marks the current thread as being within the dynamic extent of an
/// executor.
///
/// Executor implementations should call this function before blocking the
/// thread.
///
/// # Error
///
/// Returns an error if the current thread is already marked
pub fn enter() -> Result<Enter, EnterError> {
    ENTERED.with(|c| {
        if c.get() {
            Err(EnterError { _a: () })
        } else {
            c.set(true);

            Ok(Enter { _a: () })
        }
    })
}

impl fmt::Debug for Enter {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Enter").finish()
    }
}

impl Drop for Enter {
    fn drop(&mut self) {
        ENTERED.with(|c| {
            assert!(c.get());
            c.set(false);
        });
    }
}
