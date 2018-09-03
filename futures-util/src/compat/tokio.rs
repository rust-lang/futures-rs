use crate::{future::FutureExt, try_future::TryFutureExt};
use futures_core::future::FutureObj;
use futures_core::task::{Spawn, SpawnErrorKind, SpawnObjError};
use tokio_executor::{DefaultExecutor, Executor as TokioExecutor};

/// A spawner that delegates to `tokio`'s
/// [`DefaultExecutor`](tokio_executor::DefaultExecutor), will panic if used in
/// the context of a task that is not running on `tokio`'s executor.
///
/// *NOTE* The future of this struct in `futures` is uncertain. It may be
/// deprecated before or soon after the initial 0.3 release and moved to a
/// feature in `tokio` instead.
///
/// # Examples
///
/// ```ignore
/// #![feature(async_await, await_macro, futures_api)]
/// use futures::future::{FutureExt, TryFutureExt};
/// use futures::spawn;
/// use futures::compat::TokioDefaultSpawner;
/// # let (tx, rx) = futures::channel::oneshot::channel();
///
/// let future03 = async {
///     println!("Running on the pool");
///     spawn!(async {
///         println!("Spawned!");
///         # tx.send(42).unwrap();
///     }).unwrap();
/// };
///
/// let future01 = future03
///     .unit_error() // Make it a TryFuture
///     .boxed()  // Make it Unpin
///     .compat(TokioDefaultSpawner);
///
/// tokio::run(future01);
/// # futures::executor::block_on(rx).unwrap();
/// ```
#[derive(Debug, Copy, Clone)]
pub struct TokioDefaultSpawner;

impl Spawn for TokioDefaultSpawner {
    fn spawn_obj(
        &mut self,
        future: FutureObj<'static, ()>,
    ) -> Result<(), SpawnObjError> {
        let fut = Box::new(future.unit_error().compat(*self));
        DefaultExecutor::current().spawn(fut).map_err(|err| {
            panic!(
                "tokio failed to spawn and doesn't return the future: {:?}",
                err
            )
        })
    }

    fn status(&self) -> Result<(), SpawnErrorKind> {
        DefaultExecutor::current().status().map_err(|err| {
            if err.is_shutdown() {
                SpawnErrorKind::shutdown()
            } else {
                panic!(
                    "tokio executor failed for non-shutdown reason: {:?}",
                    err
                )
            }
        })
    }
}
