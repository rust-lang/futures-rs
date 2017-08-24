//! Executors
//!
//! This module contains tools for managing the raw execution of futures,
//! which is needed when building *executors* (places where futures can run).
//!
//! More information about executors can be [found online at tokio.rs][online].
//!
//! [online]: https://tokio.rs/docs/going-deeper-futures/tasks/

#[allow(deprecated)]

pub use task_impl::{Spawn, spawn, Notify, with_notify};
pub use task_impl::{UnsafeNotify, NotifyHandle};

#[cfg(feature = "use_std")]
pub use self::std_support::*;

#[cfg(feature = "use_std")]
#[allow(missing_docs)]
mod std_support {
    use std::prelude::v1::*;
    use std::cell::{RefCell, Cell};
    use std::fmt;

    #[allow(deprecated)]
    pub use task_impl::{Unpark, Executor, Run};

    thread_local!(static ENTERED: Cell<bool> = Cell::new(false));

    pub struct Enter {
        on_exit: RefCell<Vec<Box<Callback>>>,
        permanent: bool,
    }

    /// Marks the current thread as being within the dynamic extent of an
    /// executor.
    ///
    /// # Panics
    ///
    /// Panics if the current thread is *already* marked.
    pub fn enter() -> Enter {
        ENTERED.with(|c| {
            if c.get() {
                panic!("cannot reenter an executor context");
            }
            c.set(true);
        });

        Enter {
            on_exit: RefCell::new(Vec::new()),
            permanent: false,
        }
    }

    impl Enter {
        /// Register a callback to be invoked if and when the thread
        /// ceased to act as an executor.
        pub fn on_exit<F>(&self, f: F) where F: FnOnce() + 'static {
            self.on_exit.borrow_mut().push(Box::new(f));
        }

        /// Treat the remainder of execution on this thread as part of an
        /// executor; used mostly for thread pool worker threads.
        ///
        /// All registered `on_exit` callbacks are *dropped* without being
        /// invoked.
        pub fn make_permanent(mut self) {
            self.permanent = true;
        }
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

                let mut on_exit = self.on_exit.borrow_mut();
                for callback in on_exit.drain(..) {
                    callback.call();
                }
                if !self.permanent {
                    c.set(false);
                }
            });
        }
    }

    trait Callback: 'static {
        fn call(self: Box<Self>);
    }

    impl<F: FnOnce() + 'static> Callback for F {
        fn call(self: Box<Self>) {
            (*self)()
        }
    }
}
