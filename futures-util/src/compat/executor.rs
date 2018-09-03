
use super::Compat;
use crate::{TryFutureExt, FutureExt, future::UnitError};
use futures::future::Executor as Executor01;
use futures_core::task::Spawn as Spawn03;
use futures_core::task as task03;
use futures_core::future::FutureObj;

/// A future that can run on a futures 0.1
/// [`Executor`](futures::future::Executor).
pub type Executor01Future = Compat<UnitError<FutureObj<'static, ()>>, Box<dyn Spawn03 + Send>>;

/// Extension trait for futures 0.1 [`Executor`](futures::future::Executor).
pub trait Executor01CompatExt: Executor01<Executor01Future> +
                               Clone + Send + 'static
{
    /// Converts a futures 0.1 [`Executor`](futures::future::Executor) into a
    /// futures 0.3 [`Spawn`](futures_core::task::Spawn).
    ///
    /// ```ignore
    /// #![feature(async_await, await_macro, futures_api)]
    /// use futures::Future;
    /// use futures::future::{FutureExt, TryFutureExt};
    /// use futures::compat::Executor01CompatExt;
    /// use futures::spawn;
    /// use tokio_threadpool::ThreadPool;
    ///
    /// let pool01 = ThreadPool::new();
    /// let spawner03 = pool01.sender().clone().compat();
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
    /// let future01 = future03.unit_error().boxed().compat(spawner03);
    ///
    /// pool01.spawn(future01);
    /// pool01.shutdown_on_idle().wait().unwrap();
    /// # futures::executor::block_on(rx).unwrap();
    /// ```
    fn compat(self) -> Executor01As03<Self>
        where Self: Sized;
}

impl<Ex> Executor01CompatExt for Ex
where Ex: Executor01<Executor01Future> + Clone + Send + 'static
{
    fn compat(self) -> Executor01As03<Self> {
        Executor01As03 {
            executor01: self,
        }
    }
}

/// Converts a futures 0.1 [`Executor`](futures::future::Executor) into a
/// futures 0.3 [`Spawn`](futures_core::task::Spawn).
#[derive(Clone)]
pub struct Executor01As03<Ex> {
    executor01: Ex
}

impl<Ex> Spawn03 for Executor01As03<Ex>
where Ex: Executor01<Executor01Future>,
      Ex: Clone + Send + 'static,
{
    fn spawn_obj(
        &mut self,
        future: FutureObj<'static, ()>,
    ) -> Result<(), task03::SpawnObjError> {
        let exec: Box<dyn Spawn03 + Send> = Box::new(self.clone());
        let future = future.unit_error().compat(exec);

        match self.executor01.execute(future) {
            Ok(()) => Ok(()),
            Err(err) => {
                use futures_core::task::{SpawnObjError, SpawnErrorKind};

                let fut = err.into_future().into_inner().unwrap_or_else(|_| ());
                Err(SpawnObjError {
                    kind: SpawnErrorKind::shutdown(),
                    future: Box::new(fut).into(),
                })
            }
        }
    }
}
