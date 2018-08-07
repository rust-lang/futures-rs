use super::panic_executor::PanicExecutor;
use futures::task::{self, Wake};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

pub struct CounterWaker(AtomicUsize);

impl CounterWaker {
    pub fn get(&self) -> usize {
        self.0.load(Ordering::SeqCst)
    }

    pub fn set(&self, x: usize) {
        self.0.store(x, Ordering::SeqCst)
    }
}

pub fn with_counter_waker_context<F, R>(f: F) -> R
    where F: FnOnce(&mut task::Context, &Arc<CounterWaker>) -> R
{
    impl Wake for CounterWaker {
        fn wake(arc_self: &Arc<Self>) {
            arc_self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    let counter_arc = Arc::new(CounterWaker(AtomicUsize::new(0)));
    let counter_waker = unsafe { task::local_waker_ref(&counter_arc) };
    let exec = &mut PanicExecutor;

    let cx = &mut task::Context::new(&counter_waker, exec);
    f(cx, &counter_arc)
}
