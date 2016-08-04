extern crate futures;

use std::cell::Cell;
use std::sync::Arc;

use futures::{Future, Task, Poll};
use futures::executor::{Executor, ExecuteCallback};

thread_local!(static EXECUTOR_HIT: Cell<bool> = Cell::new(false));

struct MyFuture {
    executor: Arc<Executor>,
}

impl Future for MyFuture {
    type Item = ();
    type Error = ();

    fn poll(&mut self, task: &mut Task) -> Poll<(), ()> {
        if EXECUTOR_HIT.with(|p| p.get()) {
            Poll::Ok(())
        } else {
            task.poll_on(self.executor.clone());
            Poll::NotReady
        }
    }

    fn schedule(&mut self, task: &mut Task) {
        panic!("can't schedule");
    }
}

struct MyExecutor;

impl Executor for MyExecutor {
    fn execute_boxed(&self, f: Box<ExecuteCallback>) {
        EXECUTOR_HIT.with(|p| p.set(true));
        f.call();
    }
}

#[test]
fn simple() {
    let f = MyFuture { executor: Arc::new(MyExecutor) };

    f.forget();

    assert!(EXECUTOR_HIT.with(|p| p.get()));
}
