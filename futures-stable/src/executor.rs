use std::boxed::PinBox;

use futures_core::{Future, Never};
use futures_core::executor::{Executor, SpawnError};
use futures_executor::{ThreadPool, LocalPool, LocalExecutor};

use StableFuture;
use UnsafePin;

pub trait StableExecutor: Executor {
    fn spawn_pinned(&mut self, f: PinBox<Future<Item = (), Error = Never> + Send>) -> Result<(), SpawnError>;
}

impl StableExecutor for ThreadPool {
    fn spawn_pinned(&mut self, f: PinBox<Future<Item = (), Error = Never> + Send>) -> Result<(), SpawnError> {
        unsafe { self.spawn(PinBox::unpin(f)) }
    }
}

impl StableExecutor for LocalExecutor {
    fn spawn_pinned(&mut self, f: PinBox<Future<Item = (), Error = Never> + Send>) -> Result<(), SpawnError> {
        unsafe { self.spawn(PinBox::unpin(f)) }
    }
}

pub fn block_on_stable<F: StableFuture>(f: F) -> Result<F::Item, F::Error> {
    let mut pool = LocalPool::new();
    let mut exec = pool.executor();

    // run our main future to completion
    let res = pool.run_until(unsafe { UnsafePin::new(f) }, &mut exec);
    // run any remainingspawned tasks to completion
    pool.run(&mut exec);

    res
}
