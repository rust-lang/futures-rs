
use futures::future::Executor as Executor01;

use futures_core::task::Executor as Executor03;
use futures_core::task as task03;
use futures_core::future::FutureObj;

use super::Compat;
use crate::{TryFutureExt, FutureExt, future::NeverError};

pub struct BoxedExecutor(Box<dyn Executor03 + Send>);

impl Executor03 for BoxedExecutor {
    fn spawn_obj(&mut self, future: FutureObj<'static, ()>) -> Result<(), task03::SpawnObjError> {
        (&mut *self.0).spawn_obj(future)
    }
}

/// A future that can run on a futures 0.1 executor.
pub type ExecutorFuture01 = Compat<NeverError<FutureObj<'static, ()>>, BoxedExecutor>;

/// Extension trait for futures 0.1 Executors.
pub trait Executor01CompatExt: Executor01<ExecutorFuture01> 
    + Clone + Send + 'static
{
    /// Creates an `Executor` compatable with futures 0.3.
    fn compat(self) -> CompatExecutor<Self> 
        where Self: Sized;
}

impl<E> Executor01CompatExt for E
where E: Executor01<ExecutorFuture01>,
      E: Clone + Send + 'static
{
    fn compat(self) -> CompatExecutor<Self> {
        CompatExecutor {
            exec: self,
        }
    }
}

/// Converts a futures 0.1 `Executor` into a futures 0.3 `Executor`.
#[derive(Clone)]
pub struct CompatExecutor<E> {
    exec: E
}

impl<E> Executor03 for CompatExecutor<E> 
    where E: Executor01<ExecutorFuture01>,
    E: Clone + Send + 'static,
{
    fn spawn_obj(
        &mut self, 
        future: FutureObj<'static, ()>,
    ) -> Result<(), task03::SpawnObjError> {
        
        let fut = future.never_error().compat(BoxedExecutor(Box::new(self.clone())));

        self.exec.execute(fut)
            .map_err(|exec_err| {
                use futures_core::task::{SpawnObjError, SpawnErrorKind};
                     
                let fut = exec_err.into_future().into_inner().unwrap_or_else(|_| ());
                SpawnObjError {
                    kind: SpawnErrorKind::shutdown(),
                    task: Box::new(fut).into(),   
                }
            })
    }
}
