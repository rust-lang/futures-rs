use std::boxed::Box;

use futures_core::Future;
use futures_core::executor::{Executor, SpawnError};
use futures_core::never::Never;

use local_pool::LocalPool;

/// Run a future to completion on the current thread.
///
/// This function will block the caller until the given future has completed.
///
/// Use a [`LocalPool`](super::LocalPool) if you need finer-grained control over
/// spawned tasks.
///
/// # Panics
///
/// This should not be used to block on more complicated `Future`s that require
/// spawning additional tasks. This will panic when they try to do so. You
/// should consider a more complete executor situation for those kinds of
/// tasks.
pub fn block_on<F: Future>(f: F) -> Result<F::Item, F::Error> {
    let mut pool = LocalPool::new();
    pool.run_until(f, &mut BlockOnBackgroundExecutor)
}

struct BlockOnBackgroundExecutor;

impl Executor for BlockOnBackgroundExecutor {
    fn spawn(&mut self, _: Box<Future<Item = (), Error = Never> + Send>) -> Result<(), SpawnError> {
        panic!("block_on cannot be used with Futures that need to spawn additional tasks")
    }

    fn status(&self) -> Result<(), SpawnError> {
        Err(SpawnError::shutdown())
    }
}
