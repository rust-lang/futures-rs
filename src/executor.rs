//! Work in progress implementation of executors for Futures.
//!
//! Note that this interface is very likely to change and not stay as-is, and it
//! is not currently used much by futures beyond `DEFAULT`.

use std::cell::{Cell, RefCell};
use std::sync::Arc;

/// Encapsulation of a value which has the ability to execute arbitrary code.
///
/// This trait is object safe and intended to be used through pointers like
/// `Box` and `Arc.
pub trait Executor: Send + Sync + 'static {
    /// Executes the given closure `f`, perhaps on a different thread or
    /// deferred to a later time.
    ///
    /// This method may not execute `f` immediately, but it will arrange for the
    /// callback to be invoked "in the near future".
    fn execute<F>(&self, f: F)
        where F: FnOnce() + Send + 'static,
              Self: Sized
    {
        self.execute_boxed(Box::new(f))
    }

    /// Object-safe method of the above interface used when implementing trait
    /// objects.
    ///
    /// This should not be called direclty and instead `execute` should be used.
    fn execute_boxed(&self, f: Box<ExecuteCallback>);
}

/// The default executor, used by futures by default currently.
pub static DEFAULT: Limited = Limited;

impl<T: Executor + ?Sized + Send + Sync + 'static> Executor for Box<T> {
    fn execute_boxed(&self, f: Box<ExecuteCallback>) {
        (**self).execute_boxed(f)
    }
}

impl<T: Executor + ?Sized + Send + Sync + 'static> Executor for Arc<T> {
    fn execute_boxed(&self, f: Box<ExecuteCallback>) {
        (**self).execute_boxed(f)
    }
}

/// Essentially `Box<FnOnce() + Send>`, just as a trait.
pub trait ExecuteCallback: Send + 'static {
    #[allow(missing_docs)]
    fn call(self: Box<Self>);
}

impl<F: FnOnce() + Send + 'static> ExecuteCallback for F {
    fn call(self: Box<F>) {
        (*self)()
    }
}

/// Implementation of an `Executor` which just executes everything immediately
/// as soon as it's passed in.
pub struct Inline;

impl Executor for Inline {
    fn execute<F: FnOnce() + Send + 'static>(&self, f: F) {
        f()
    }

    fn execute_boxed(&self, f: Box<ExecuteCallback>) {
        f.call()
    }
}

/// Implementation of an executor which executes all callbacks immediately, but
/// bounds the amount of recursion to prevent blowing the stack.
pub struct Limited;

thread_local!(static LIMITED: LimitState = LimitState::new());

const LIMIT: usize = 100;

struct LimitState {
    count: Cell<usize>,
    deferred: RefCell<Vec<Box<ExecuteCallback>>>,
}

impl Executor for Limited {
    fn execute<F>(&self, f: F) where F: FnOnce() + Send + 'static {
        LIMITED.with(|state| state.execute(f))
    }
    fn execute_boxed(&self, f: Box<ExecuteCallback>) {
        self.execute(|| f.call());
    }
}

impl LimitState {
    fn new() -> LimitState {
        LimitState {
            count: Cell::new(0),
            deferred: RefCell::new(Vec::new()),
        }
    }

    fn execute<F>(&self, f: F) where F: FnOnce() + Send + 'static {
        match self.count.get() {
            0 => {
                self.count.set(1);
                f();
                loop {
                    let cb = self.deferred.borrow_mut().pop();
                    match cb {
                        Some(f) => f.call(),
                        None => break,
                    }
                }
                self.count.set(0);
            }
            n if n < LIMIT => {
                self.count.set(n + 1);
                f();
                self.count.set(n);
            }
            _ => self.deferred.borrow_mut().push(Box::new(f)),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::{Executor, Limited};

    #[test]
    fn limited() {
        fn doit(ex: Arc<Executor>, hits: Arc<AtomicUsize>, i: usize) {
            if i == 0 {
                return
            }
            hits.fetch_add(1, Ordering::SeqCst);
            let ex2 = ex.clone();
            ex.execute(move || {
                doit(ex2, hits, i - 1);
            })
        }

        let n = 1_000_000;
        let hits = Arc::new(AtomicUsize::new(0));
        doit(Arc::new(Limited), hits.clone(), n);
        assert_eq!(hits.load(Ordering::SeqCst), n);
    }
}
