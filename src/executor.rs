use std::cell::{Cell, RefCell};
use std::sync::Arc;

pub trait Executor: Send + Sync + 'static {
    fn execute<F>(&self, f: F)
        where F: FnOnce() + Send + 'static,
              Self: Sized
    {
        self.execute_boxed(Box::new(f))
    }

    fn execute_boxed(&self, f: Box<ExecuteCallback>);
}

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

pub trait ExecuteCallback: Send + 'static {
    fn call(self: Box<Self>);
}

impl<F: FnOnce() + Send + 'static> ExecuteCallback for F {
    fn call(self: Box<F>) {
        (*self)()
    }
}

pub struct Default;

impl Executor for Default {
    fn execute<F: FnOnce() + Send + 'static>(&self, f: F) {
        f()
    }

    fn execute_boxed(&self, f: Box<ExecuteCallback>) {
        f.call()
    }
}

pub struct Limited;

thread_local!(static LIMITED: LimitState = LimitState::new());

const LIMIT: usize = 100;

struct LimitState {
    count: Cell<usize>,
    deferred: RefCell<Vec<Box<ExecuteCallback>>>,
}

impl Executor for Limited {
    fn execute_boxed(&self, f: Box<ExecuteCallback>) {
        LIMITED.with(|state| state.execute(f))
    }
}

impl LimitState {
    fn new() -> LimitState {
        LimitState {
            count: Cell::new(0),
            deferred: RefCell::new(Vec::new()),
        }
    }

    fn execute(&self, cb: Box<ExecuteCallback>) {
        match self.count.get() {
            0 => {
                self.count.set(1);
                let mut cb = Some(cb);
                while let Some(f) = cb {
                    f.call();
                    cb = self.deferred.borrow_mut().pop();
                }
                self.count.set(0);
            }
            n if n < LIMIT => {
                self.count.set(n + 1);
                cb.call();
                self.count.set(n);
            }
            _ => self.deferred.borrow_mut().push(cb),
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
